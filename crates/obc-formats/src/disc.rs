//! Reading the game off a CD image: a BIN/CUE pair (the Steam and GOG
//! release) or a plain ISO (docs/DECISIONS.md, ADR-022).
//!
//! Ported from the EC7Wolf port of Corridor 7 (`EC7Wolf/tools/c7disc.py`), which
//! reads its disc the same way. Only what a 1990s ISO9660 level-1 disc uses:
//! the primary volume descriptor and directory records, no Joliet or Rock
//! Ridge. File names are returned bare and upper-case, without the `;1`
//! version suffix or a directory path; the first (shallowest) file of a name
//! wins.
//!
//! Sector layouts: `MODE1/2048` and ISO images hold 2048-byte sectors;
//! `MODE1/2352` sectors carry a 16-byte sync and header before the data
//! (and 288 bytes of error correction after); `MODE2/2352` a 24-byte header;
//! `MODE2/2336` an 8-byte subheader. Audio tracks are raw Red Book: 2352
//! bytes per sector, 44.1 kHz 16-bit little-endian stereo (big-endian for a
//! `MOTOROLA` file).

use std::collections::BTreeMap;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use crate::error::{FormatError, Result};

const DATA: u64 = 2048;
const RAW: u64 = 2352;
/// Sectors per second, also CUE "frames".
pub const SECTORS_PER_SECOND: u64 = 75;
/// Stereo samples per audio sector.
pub const SAMPLES_PER_SECTOR: usize = 588;
/// Red Book sample rate.
pub const AUDIO_RATE: u32 = 44_100;

fn invalid(msg: impl Into<String>) -> FormatError {
    FormatError::Invalid { what: "disc image", msg: msg.into() }
}

fn io(path: &Path) -> impl FnOnce(std::io::Error) -> FormatError + '_ {
    move |source| FormatError::Io { path: path.to_owned(), source }
}

/// A run of logical 2048-byte sectors inside an image file.
#[derive(Debug, Clone)]
struct DataTrack {
    file: PathBuf,
    /// First sector of the track within the file.
    first: u64,
    /// Sector size in the file, and where the 2048 data bytes start in one.
    step: u64,
    skip: u64,
}

impl DataTrack {
    fn read(&self, f: &mut File, lba: u64, len: u64) -> Result<Vec<u8>> {
        let mut out = vec![0; len as usize];
        if self.step == DATA {
            f.seek(SeekFrom::Start((self.first + lba) * DATA)).map_err(io(&self.file))?;
            f.read_exact(&mut out).map_err(io(&self.file))?;
            return Ok(out);
        }
        for (i, chunk) in out.chunks_mut(DATA as usize).enumerate() {
            let at = (self.first + lba + i as u64) * self.step + self.skip;
            f.seek(SeekFrom::Start(at)).map_err(io(&self.file))?;
            f.read_exact(chunk).map_err(io(&self.file))?;
        }
        Ok(out)
    }
}

/// One Red Book audio track in an image.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AudioTrack {
    /// Track number on the disc (the data track is 1).
    pub number: u8,
    file: PathBuf,
    first: u64,
    /// Length in sectors (1/75 s).
    pub sectors: u64,
    big_endian: bool,
}

impl AudioTrack {
    pub fn seconds(&self) -> f64 {
        self.sectors as f64 / SECTORS_PER_SECOND as f64
    }

    /// Stereo samples in the track.
    pub fn samples(&self) -> u64 {
        self.sectors * SAMPLES_PER_SECTOR as u64
    }
}

/// A CD image whose data track has been indexed.
#[derive(Debug, Clone)]
pub struct Disc {
    source: PathBuf,
    data: DataTrack,
    files: BTreeMap<String, (u64, u64)>,
    audio: Vec<AudioTrack>,
}

impl Disc {
    /// Opens a `.cue`, a `.bin`/`.img` with its `.cue` beside it, or an `.iso`.
    pub fn open(path: &Path) -> Result<Self> {
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
        match ext.as_str() {
            "cue" => Self::open_cue(path),
            "bin" | "img" => Self::open_cue(&cue_beside(path)?),
            _ => {
                let data = DataTrack { file: path.to_owned(), first: 0, step: DATA, skip: 0 };
                Self::index(path, data, Vec::new())
            }
        }
    }

    /// True if `path` names something [`Disc::open`] would try.
    pub fn is_image(path: &Path) -> bool {
        path.is_file()
            && path.extension().and_then(|e| e.to_str()).is_some_and(|e| {
                matches!(e.to_ascii_lowercase().as_str(), "cue" | "bin" | "img" | "iso")
            })
    }

    fn open_cue(cue: &Path) -> Result<Self> {
        let text = std::fs::read(cue).map_err(io(cue))?;
        let sheet = parse_cue(&String::from_utf8_lossy(&text))?;
        let dir = cue.parent().unwrap_or(Path::new("."));
        let mut data = None;
        let mut audio = Vec::new();
        for (file_name, big_endian, tracks) in &sheet {
            let file = resolve(dir, file_name)?;
            let total = std::fs::metadata(&file).map_err(io(&file))?.len() / RAW;
            // A PREGAP is silence the disc has and the file does not, so every
            // later track in the file sits that much earlier than its INDEX
            // says, and the shift accumulates (c7disc.py, CueSource).
            let mut shift = 0;
            let mut placed = Vec::new();
            for t in tracks {
                shift += t.pregap;
                placed.push((
                    t,
                    t.index01.saturating_sub(shift),
                    t.index00.map(|i| i.saturating_sub(shift)),
                ));
            }
            for (i, &(t, start, _)) in placed.iter().enumerate() {
                let end = placed.get(i + 1).map_or(total, |&(_, s, s0)| s0.unwrap_or(s));
                let sectors = end.saturating_sub(start);
                if t.mode == "AUDIO" {
                    audio.push(AudioTrack {
                        number: t.number,
                        file: file.clone(),
                        first: start,
                        sectors,
                        big_endian: *big_endian,
                    });
                } else if data.is_none() {
                    let (step, skip) = match t.mode.as_str() {
                        "MODE1/2048" => (DATA, 0),
                        "MODE1/2352" => (RAW, 16),
                        "MODE2/2352" => (RAW, 24),
                        "MODE2/2336" => (2336, 8),
                        m => return Err(invalid(format!("unsupported track mode {m}"))),
                    };
                    data = Some(DataTrack { file: file.clone(), first: start, step, skip });
                }
            }
        }
        let data = data.ok_or_else(|| invalid(format!("{} has no data track", cue.display())))?;
        Self::index(cue, data, audio)
    }

    fn index(source: &Path, data: DataTrack, audio: Vec<AudioTrack>) -> Result<Self> {
        let mut f = File::open(&data.file).map_err(io(&data.file))?;
        let mut pvd = None;
        for lba in 16..32 {
            let Ok(block) = data.read(&mut f, lba, DATA) else { break };
            if &block[1..6] != b"CD001" {
                continue;
            }
            match block[0] {
                1 => {
                    pvd = Some(block);
                    break;
                }
                255 => break,
                _ => {}
            }
        }
        let pvd = pvd.ok_or_else(|| invalid("no ISO9660 primary volume descriptor"))?;
        let le32 =
            |b: &[u8], at: usize| u32::from_le_bytes([b[at], b[at + 1], b[at + 2], b[at + 3]]);
        let mut queue = std::collections::VecDeque::from([(
            u64::from(le32(&pvd, 156 + 2)),
            u64::from(le32(&pvd, 156 + 10)),
        )]);
        let mut seen = std::collections::BTreeSet::new();
        let mut files = BTreeMap::new();
        while let Some((lba, len)) = queue.pop_front() {
            if len == 0 || len > 1 << 24 || !seen.insert((lba, len)) {
                continue;
            }
            let dir = data.read(&mut f, lba, len.div_ceil(DATA) * DATA)?;
            let mut at = 0;
            while at < dir.len() {
                let rec_len = usize::from(dir[at]);
                if rec_len == 0 {
                    // Records never straddle a sector boundary.
                    at = (at / DATA as usize + 1) * DATA as usize;
                    continue;
                }
                let Some(rec) = dir.get(at..at + rec_len).filter(|r| r.len() >= 33) else { break };
                let name_len = usize::from(rec[32]);
                let name = rec.get(33..33 + name_len).unwrap_or_default();
                let (child, child_len) = (u64::from(le32(rec, 2)), u64::from(le32(rec, 10)));
                if rec[25] & 2 != 0 {
                    if name != [0] && name != [1] {
                        queue.push_back((child, child_len));
                    }
                } else {
                    let name = String::from_utf8_lossy(name);
                    let bare = name.split(';').next().unwrap_or("").to_ascii_uppercase();
                    let bare = bare.strip_suffix('.').map(str::to_owned).unwrap_or(bare);
                    files.entry(bare).or_insert((child, child_len));
                }
                at += rec_len;
            }
        }
        Ok(Self { source: source.to_owned(), data, files, audio })
    }

    /// The file the disc was opened from.
    pub fn source(&self) -> &Path {
        &self.source
    }

    /// Upper-case names of the files on the disc.
    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.files.keys().map(String::as_str)
    }

    pub fn size(&self, name: &str) -> Option<u64> {
        self.files.get(&name.to_ascii_uppercase()).map(|&(_, len)| len)
    }

    /// Reads the whole file `name` (any case).
    pub fn read(&self, name: &str) -> Result<Vec<u8>> {
        let &(lba, len) = self
            .files
            .get(&name.to_ascii_uppercase())
            .ok_or_else(|| FormatError::Missing(name.to_owned()))?;
        let mut f = File::open(&self.data.file).map_err(io(&self.data.file))?;
        let mut out = self.data.read(&mut f, lba, len.div_ceil(DATA) * DATA)?;
        out.truncate(len as usize);
        Ok(out)
    }

    pub fn audio_tracks(&self) -> &[AudioTrack] {
        &self.audio
    }

    /// Streams `track` as interleaved stereo 16-bit samples, about a second at
    /// a time. `sink` returns `false` to stop early.
    pub fn read_audio(
        &self,
        track: &AudioTrack,
        mut sink: impl FnMut(&[i16]) -> bool,
    ) -> Result<()> {
        let mut f = File::open(&track.file).map_err(io(&track.file))?;
        f.seek(SeekFrom::Start(track.first * RAW)).map_err(io(&track.file))?;
        let mut raw = vec![0u8; (SECTORS_PER_SECOND * RAW) as usize];
        let mut pcm = Vec::with_capacity(raw.len() / 2);
        let mut left = track.sectors;
        while left > 0 {
            let n = left.min(SECTORS_PER_SECOND);
            let buf = &mut raw[..(n * RAW) as usize];
            f.read_exact(buf).map_err(io(&track.file))?;
            pcm.clear();
            pcm.extend(buf.chunks_exact(2).map(|b| {
                if track.big_endian {
                    i16::from_be_bytes([b[0], b[1]])
                } else {
                    i16::from_le_bytes([b[0], b[1]])
                }
            }));
            if !sink(&pcm) {
                break;
            }
            left -= n;
        }
        Ok(())
    }
}

impl Disc {
    /// Encodes `track` as a QOA file into `out`, streaming. `progress` gets
    /// the fraction done (0..=1) about once a second of audio.
    pub fn rip_qoa(
        &self,
        track: &AudioTrack,
        out: &mut impl std::io::Write,
        mut progress: impl FnMut(f64),
    ) -> Result<()> {
        use crate::qoa::{Encoder, FRAME_LEN};
        let total = track.samples();
        let mut enc = Encoder::new(2, AUDIO_RATE)?;
        let mut bytes = Vec::with_capacity(FRAME_LEN);
        enc.header(total as u32, &mut bytes);
        let mut pending: Vec<i16> = Vec::with_capacity(FRAME_LEN * 4);
        let mut done = 0u64;
        let mut failed = None;
        let path = self.source.clone();
        let mut write = |bytes: &mut Vec<u8>| -> Result<()> {
            out.write_all(bytes)
                .map_err(|source| FormatError::Io { path: path.clone(), source })?;
            bytes.clear();
            Ok(())
        };
        write(&mut bytes)?;
        self.read_audio(track, |pcm| {
            pending.extend_from_slice(pcm);
            while pending.len() >= FRAME_LEN * 2 {
                enc.frame(&pending[..FRAME_LEN * 2], &mut bytes);
                pending.drain(..FRAME_LEN * 2);
            }
            if let Err(e) = write(&mut bytes) {
                failed = Some(e);
                return false;
            }
            done += (pcm.len() / 2) as u64;
            progress(done as f64 / total.max(1) as f64);
            true
        })?;
        if let Some(e) = failed {
            return Err(e);
        }
        if !pending.is_empty() {
            enc.frame(&pending, &mut bytes);
        }
        write(&mut bytes)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CueTrack {
    number: u8,
    mode: String,
    index00: Option<u64>,
    index01: u64,
    pregap: u64,
}

fn msf(s: &str) -> Option<u64> {
    let mut it = s.split(':').map(|p| p.parse::<u64>().ok());
    let (m, sec, f) = (it.next()??, it.next()??, it.next()??);
    Some((m * 60 + sec) * SECTORS_PER_SECOND + f)
}

/// Splits a cue line into words, keeping quoted strings whole.
fn words(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut quoted = false;
    for c in line.chars() {
        match c {
            '"' => quoted = !quoted,
            c if c.is_whitespace() && !quoted => {
                if !cur.is_empty() {
                    out.push(std::mem::take(&mut cur));
                }
            }
            c => cur.push(c),
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

/// `(file name, big-endian audio, tracks)` per `FILE` line.
type CueSheet = Vec<(String, bool, Vec<CueTrack>)>;

fn parse_cue(text: &str) -> Result<CueSheet> {
    let mut sheet: CueSheet = Vec::new();
    for line in text.lines() {
        let w = words(line);
        let Some(cmd) = w.first() else { continue };
        let arg = |i: usize| w.get(i).map(String::as_str).unwrap_or("");
        match cmd.to_ascii_uppercase().as_str() {
            "FILE" => {
                let kind = arg(2).to_ascii_uppercase();
                if !matches!(kind.as_str(), "BINARY" | "MOTOROLA") {
                    return Err(invalid(format!("unsupported FILE type {kind} for {}", arg(1))));
                }
                sheet.push((arg(1).to_owned(), kind == "MOTOROLA", Vec::new()));
            }
            "TRACK" => {
                let (_, _, tracks) =
                    sheet.last_mut().ok_or_else(|| invalid("TRACK before FILE"))?;
                let number =
                    arg(1).parse().map_err(|_| invalid(format!("bad track {}", arg(1))))?;
                tracks.push(CueTrack {
                    number,
                    mode: arg(2).to_ascii_uppercase(),
                    index00: None,
                    index01: u64::MAX,
                    pregap: 0,
                });
            }
            "INDEX" | "PREGAP" => {
                let t = sheet
                    .last_mut()
                    .and_then(|(_, _, t)| t.last_mut())
                    .ok_or_else(|| invalid(format!("{cmd} before TRACK")))?;
                let (which, time) = if cmd.eq_ignore_ascii_case("PREGAP") {
                    ("PREGAP", arg(1))
                } else {
                    (arg(1), arg(2))
                };
                let at = msf(time).ok_or_else(|| invalid(format!("bad time {time}")))?;
                match which {
                    "PREGAP" => t.pregap = at,
                    "00" | "0" => t.index00 = Some(at),
                    "01" | "1" => t.index01 = at,
                    _ => {}
                }
            }
            _ => {}
        }
    }
    for (name, _, tracks) in &sheet {
        if let Some(t) = tracks.iter().find(|t| t.index01 == u64::MAX) {
            return Err(invalid(format!("track {} in {name} has no INDEX 01", t.number)));
        }
    }
    if sheet.is_empty() {
        return Err(invalid("no FILE line"));
    }
    Ok(sheet)
}

/// Finds `name` beside the cue sheet, ignoring case and any directory part.
fn resolve(dir: &Path, name: &str) -> Result<PathBuf> {
    let direct = dir.join(name);
    if direct.is_file() {
        return Ok(direct);
    }
    let bare = Path::new(name).file_name().map(|n| n.to_string_lossy().to_ascii_lowercase());
    if let (Some(bare), Ok(entries)) = (bare, std::fs::read_dir(dir)) {
        for e in entries.flatten() {
            if e.file_name().to_string_lossy().to_ascii_lowercase() == bare {
                return Ok(e.path());
            }
        }
    }
    Err(invalid(format!("the cue sheet names {name}, which is not beside it")))
}

/// The `.cue` for a `.bin`: same name, any case, in the same folder.
fn cue_beside(bin: &Path) -> Result<PathBuf> {
    let stem = bin.file_stem().map(|s| s.to_string_lossy().to_ascii_lowercase());
    let dir = bin.parent().unwrap_or(Path::new("."));
    if let (Some(stem), Ok(entries)) = (stem, std::fs::read_dir(dir)) {
        for e in entries.flatten() {
            let p = e.path();
            let is_cue = p.extension().is_some_and(|x| x.eq_ignore_ascii_case("cue"));
            let same =
                p.file_stem().is_some_and(|s| s.to_string_lossy().to_ascii_lowercase() == stem);
            if is_cue && same {
                return Ok(p);
            }
        }
    }
    Err(invalid(format!(
        "{} is a raw image; the .cue beside it says how its tracks are laid out, and none was found. Choose the .cue file.",
        bin.display()
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_the_steam_sheet() {
        let sheet = parse_cue(
            "FILE \"BODYCOUNT.BIN\" BINARY\n  TRACK 01 MODE1/2352\n    INDEX 01 00:00:00\n  TRACK 02 AUDIO\n    INDEX 01 06:12:52\n  TRACK 03 AUDIO\n    PREGAP 00:02:00\n    INDEX 00 14:00:00\n    INDEX 01 14:01:68\n",
        )
        .unwrap();
        assert_eq!(sheet.len(), 1);
        let (name, be, t) = &sheet[0];
        assert_eq!((name.as_str(), *be, t.len()), ("BODYCOUNT.BIN", false, 3));
        assert_eq!(t[1].index01, (6 * 60 + 12) * 75 + 52);
        assert_eq!((t[2].pregap, t[2].index00), (150, Some(14 * 60 * 75)));
    }

    #[test]
    fn rejects_bad_sheets() {
        assert!(parse_cue("TRACK 01 AUDIO\n").is_err());
        assert!(parse_cue("FILE \"a.wav\" WAVE\n").is_err());
        assert!(parse_cue("FILE \"a.bin\" BINARY\nTRACK 01 AUDIO\n").is_err());
        assert!(parse_cue("").is_err());
    }

    /// A tiny MODE1/2352 image: PVD at 16, root directory at 18 with one file
    /// at 20, then a one-sector audio track.
    #[test]
    fn reads_files_and_audio_from_a_bin_cue() {
        let dir = std::env::temp_dir().join(format!("obcr-disc-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut sectors = vec![[0u8; 2048]; 21];
        sectors[16][0] = 1;
        sectors[16][1..6].copy_from_slice(b"CD001");
        let root = |lba: u32, len: u32, flags: u8, name: &[u8]| {
            let mut r = vec![0u8; 33 + name.len() + (1 - name.len() % 2)];
            r[0] = r.len() as u8;
            r[2..6].copy_from_slice(&lba.to_le_bytes());
            r[10..14].copy_from_slice(&len.to_le_bytes());
            r[25] = flags;
            r[32] = name.len() as u8;
            r[33..33 + name.len()].copy_from_slice(name);
            r
        };
        let rr = root(18, 2048, 2, &[0]);
        sectors[16][156..156 + 34].copy_from_slice(&rr[..34]);
        let mut d = Vec::new();
        d.extend(root(18, 2048, 2, &[0]));
        d.extend(root(18, 2048, 2, &[1]));
        d.extend(root(20, 5, 0, b"HELLO.TXT;1"));
        sectors[18][..d.len()].copy_from_slice(&d);
        sectors[20][..5].copy_from_slice(b"hello");
        let mut bin = Vec::new();
        for s in &sectors {
            bin.extend([0u8; 16]);
            bin.extend(s);
            bin.extend([0u8; 288]);
        }
        let audio: Vec<u8> = (0..588 * 2).flat_map(|i: i16| i.to_le_bytes()).collect();
        bin.extend(&audio);
        std::fs::write(dir.join("Test.BIN"), &bin).unwrap();
        std::fs::write(
            dir.join("test.cue"),
            "FILE \"TEST.bin\" BINARY\nTRACK 01 MODE1/2352\nINDEX 01 00:00:00\nTRACK 02 AUDIO\nINDEX 01 00:00:21\n",
        )
        .unwrap();
        let disc = Disc::open(&dir.join("Test.BIN")).unwrap();
        assert_eq!(disc.names().collect::<Vec<_>>(), ["HELLO.TXT"]);
        assert_eq!(disc.read("hello.txt").unwrap(), b"hello");
        let tracks = disc.audio_tracks().to_vec();
        assert_eq!((tracks.len(), tracks[0].number, tracks[0].sectors), (1, 2, 1));
        let mut got = Vec::new();
        disc.read_audio(&tracks[0], |s| {
            got.extend_from_slice(s);
            true
        })
        .unwrap();
        assert_eq!(got, (0..588 * 2).collect::<Vec<i16>>());
        std::fs::remove_dir_all(&dir).ok();
    }
}
