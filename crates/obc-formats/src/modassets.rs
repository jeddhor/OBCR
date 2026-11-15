//! A mod's own art, sounds and music (mod format 2,
//! docs/MOD_ASSETS_AND_ARENAS.md §2).
//!
//! Names: `walls/NNN.png` (pages 0-135), `sprites/NNN.png` (0-679),
//! `sounds/NN.wav` (0-98), `music/NAME.ogg|qoa|imf`. Every file is untrusted:
//! sizes are checked before decoding, and decoding is done by memory-safe
//! decoders (`png`, `hound`, `lewton`, our QOA and IMF readers).
//!
//! Images are returned as 64×64 RGBA; converting them to the game palette is
//! the renderer's job (`obc_render::quantize`), which knows the palette.

use std::collections::BTreeMap;
use std::io::Cursor;
use std::sync::Arc;

use crate::audio::{ImfCommand, parse_imf};
use crate::stream::{PcmStream, StreamKind};

/// Wall pages a mod may replace (the original's 136).
pub const WALL_PAGES: u16 = 136;
/// Sprites a mod may replace (the original's 680).
pub const SPRITES: u16 = 680;
/// Digitized sounds a mod may replace (`AUDIOMUS.BC`'s 99).
pub const SOUNDS: u16 = 99;
/// Side of a wall page or sprite image.
pub const IMAGE_SIZE: u32 = 64;
/// Largest image file.
pub const MAX_IMAGE_BYTES: usize = 256 << 10;
/// Largest sound file and longest sound.
pub const MAX_SOUND_BYTES: usize = 4 << 20;
pub const MAX_SOUND_SECONDS: u32 = 30;
/// Sample rates a sound may have.
pub const SOUND_RATES: std::ops::RangeInclusive<u32> = 4_000..=48_000;
/// Most music files and the longest piece.
pub const MAX_MUSIC_FILES: usize = 64;
pub const MAX_MUSIC_SECONDS: u32 = 60 * 60;
/// Music sample rates the stream voice accepts.
pub const MUSIC_RATES: std::ops::RangeInclusive<u32> = 8_000..=96_000;

/// What a file in a mod is, by its name.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum AssetName {
    Wall(u16),
    Sprite(u16),
    Sound(u16),
    /// The music file's name without the folder, as written.
    Music(String),
}

/// A music file's format.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MusicFormat {
    Vorbis,
    Qoa,
    Imf,
}

fn number(stem: &str, max_digits: usize, limit: u16) -> Result<u16, String> {
    if stem.is_empty() || stem.len() > max_digits || !stem.bytes().all(|b| b.is_ascii_digit()) {
        return Err(format!("expected a number of 1-{max_digits} digits, not {stem:?}"));
    }
    let n: u16 = stem.parse().map_err(|_| format!("{stem:?} is not a number"))?;
    if n >= limit {
        return Err(format!("{n} is out of range (0-{})", limit - 1));
    }
    Ok(n)
}

/// A valid music file name (`NAME.ogg|qoa|imf`), with its format.
pub fn music_name(name: &str) -> Result<MusicFormat, String> {
    let (stem, ext) = name.rsplit_once('.').ok_or("a music file needs .ogg, .qoa or .imf")?;
    let format = match ext.to_ascii_lowercase().as_str() {
        "ogg" => MusicFormat::Vorbis,
        "qoa" => MusicFormat::Qoa,
        "imf" => MusicFormat::Imf,
        _ => return Err(format!("music {name:?}: use .ogg, .qoa or .imf")),
    };
    if !(1..=32).contains(&stem.len())
        || !stem.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
    {
        return Err(format!("music {name:?}: 1-32 characters A-Z, a-z, 0-9, _ and -"));
    }
    Ok(format)
}

/// Classifies a package path. `None`: not in an asset folder (format 1's
/// files, or something ignored). `Some(Err)`: in an asset folder but badly
/// named, which is an error rather than silently unused.
pub fn classify(path: &str) -> Option<Result<AssetName, String>> {
    let (folder, file) = path.split_once('/')?;
    let folder = folder.to_ascii_lowercase();
    if !matches!(folder.as_str(), "walls" | "sprites" | "sounds" | "music") {
        return None;
    }
    if file.is_empty() {
        return None; // a ZIP directory entry
    }
    let bad = |msg: String| Some(Err(format!("{path}: {msg}")));
    if file.contains('/') {
        return bad("asset folders have no subfolders".into());
    }
    let (stem, ext) = file.rsplit_once('.').unwrap_or((file, ""));
    let ext = ext.to_ascii_lowercase();
    let r = match folder.as_str() {
        "walls" if ext == "png" => number(stem, 3, WALL_PAGES).map(AssetName::Wall),
        "sprites" if ext == "png" => number(stem, 3, SPRITES).map(AssetName::Sprite),
        "sounds" if ext == "wav" => number(stem, 2, SOUNDS).map(AssetName::Sound),
        "music" => music_name(file).map(|_| AssetName::Music(file.to_owned())),
        "walls" | "sprites" => Err("images are .png files".into()),
        _ => Err("sounds are .wav files".into()),
    };
    match r {
        Ok(a) => Some(Ok(a)),
        Err(e) => bad(e),
    }
}

/// The asset files of a package, by what they replace: name → package path.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AssetIndex {
    pub walls: BTreeMap<u16, String>,
    pub sprites: BTreeMap<u16, String>,
    pub sounds: BTreeMap<u16, String>,
    /// Lower-case music name → package path.
    pub music: BTreeMap<String, String>,
}

impl AssetIndex {
    /// Indexes package paths. Errors: badly named files and two files for
    /// the same number or music name.
    pub fn build<'a>(paths: impl IntoIterator<Item = &'a str>) -> Result<Self, String> {
        let mut ix = Self::default();
        for p in paths {
            let Some(a) = classify(p) else { continue };
            let dup = match a? {
                AssetName::Wall(n) => ix.walls.insert(n, p.to_owned()),
                AssetName::Sprite(n) => ix.sprites.insert(n, p.to_owned()),
                AssetName::Sound(n) => ix.sounds.insert(n, p.to_owned()),
                AssetName::Music(m) => ix.music.insert(m.to_ascii_lowercase(), p.to_owned()),
            };
            if let Some(other) = dup {
                return Err(format!("{p} and {other} replace the same thing"));
            }
        }
        if ix.music.len() > MAX_MUSIC_FILES {
            return Err(format!("{} music files (at most {MAX_MUSIC_FILES})", ix.music.len()));
        }
        Ok(ix)
    }

    pub fn is_empty(&self) -> bool {
        self.walls.is_empty()
            && self.sprites.is_empty()
            && self.sounds.is_empty()
            && self.music.is_empty()
    }

    /// The package path of a music file named in a manifest.
    pub fn music_path(&self, name: &str) -> Option<&str> {
        self.music.get(&name.to_ascii_lowercase()).map(String::as_str)
    }
}

/// A decoded 64×64 image: row-major RGBA.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Image {
    pub rgba: Vec<[u8; 4]>,
}

impl Image {
    pub fn at(&self, x: usize, y: usize) -> [u8; 4] {
        self.rgba[y * IMAGE_SIZE as usize + x]
    }
}

/// Decodes a 64×64 PNG of any colour type and bit depth.
pub fn decode_png(bytes: &[u8]) -> Result<Image, String> {
    if bytes.len() > MAX_IMAGE_BYTES {
        return Err(format!("{} bytes (at most {MAX_IMAGE_BYTES})", bytes.len()));
    }
    let mut dec = png::Decoder::new(Cursor::new(bytes));
    dec.set_transformations(png::Transformations::normalize_to_color8());
    // The decoder refuses images bigger than this before allocating.
    dec.set_limits(png::Limits { bytes: 1 << 20 });
    let mut reader = dec.read_info().map_err(|e| format!("not a readable PNG ({e})"))?;
    let (w, h) = (reader.info().width, reader.info().height);
    if (w, h) != (IMAGE_SIZE, IMAGE_SIZE) {
        return Err(format!("{w}x{h} (images are {IMAGE_SIZE}x{IMAGE_SIZE})"));
    }
    let size = reader.output_buffer_size().ok_or("image too large")?;
    let mut buf = vec![0u8; size];
    let frame = reader.next_frame(&mut buf).map_err(|e| format!("damaged PNG ({e})"))?;
    let data = &buf[..frame.buffer_size()];
    let px = (IMAGE_SIZE * IMAGE_SIZE) as usize;
    let rgba: Vec<[u8; 4]> = match frame.color_type {
        png::ColorType::Rgba => data.chunks_exact(4).map(|c| [c[0], c[1], c[2], c[3]]).collect(),
        png::ColorType::Rgb => data.chunks_exact(3).map(|c| [c[0], c[1], c[2], 255]).collect(),
        png::ColorType::GrayscaleAlpha => {
            data.chunks_exact(2).map(|c| [c[0], c[0], c[0], c[1]]).collect()
        }
        png::ColorType::Grayscale => data.iter().map(|&g| [g, g, g, 255]).collect(),
        png::ColorType::Indexed => return Err("palette image was not expanded".into()),
    };
    if rgba.len() != px {
        return Err("damaged PNG (short image data)".into());
    }
    Ok(Image { rgba })
}

/// A decoded sound: 8-bit unsigned mono, the game's own sample format.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sound {
    pub rate: u32,
    pub samples: Vec<u8>,
}

/// Decodes a PCM WAV (8/16/24/32-bit integer or 32-bit float, mono or
/// stereo) to 8-bit unsigned mono at its own rate.
pub fn decode_wav(bytes: &[u8]) -> Result<Sound, String> {
    if bytes.len() > MAX_SOUND_BYTES {
        return Err(format!("{} bytes (at most {MAX_SOUND_BYTES})", bytes.len()));
    }
    let mut r = hound::WavReader::new(Cursor::new(bytes))
        .map_err(|e| format!("not a readable WAV ({e})"))?;
    let spec = r.spec();
    let ch = usize::from(spec.channels);
    if !(1..=2).contains(&ch) {
        return Err(format!("{ch} channels (mono or stereo)"));
    }
    if !SOUND_RATES.contains(&spec.sample_rate) {
        return Err(format!(
            "{} Hz ({}-{} Hz)",
            spec.sample_rate,
            SOUND_RATES.start(),
            SOUND_RATES.end()
        ));
    }
    let frames = u64::from(r.duration());
    if frames > u64::from(MAX_SOUND_SECONDS) * u64::from(spec.sample_rate) {
        return Err(format!("longer than {MAX_SOUND_SECONDS} seconds"));
    }
    // Every sample as a float in [-1, 1].
    let floats: Vec<f32> = match (spec.sample_format, spec.bits_per_sample) {
        (hound::SampleFormat::Float, 32) => r.samples::<f32>().collect::<Result<_, _>>(),
        (hound::SampleFormat::Int, b @ (8 | 16 | 24 | 32)) => {
            let scale = 1.0 / (1u64 << (b - 1)) as f32;
            r.samples::<i32>().map(|s| s.map(|v| v as f32 * scale)).collect::<Result<_, _>>()
        }
        (f, b) => return Err(format!("unsupported sample format {f:?} {b}-bit")),
    }
    .map_err(|e| format!("damaged WAV ({e})"))?;
    if floats.is_empty() {
        return Err("no samples".into());
    }
    let samples = floats
        .chunks(ch)
        .map(|f| {
            let v = f.iter().sum::<f32>() / ch as f32;
            (v.clamp(-1.0, 1.0) * 128.0 + 128.0).round().clamp(0.0, 255.0) as u8
        })
        .collect();
    Ok(Sound { rate: spec.sample_rate, samples })
}

/// Reads an IMF song: a "type 1" file (a 16-bit length, then commands, as in
/// `AUDIOT`) or a "type 0" file (commands only).
pub fn read_imf(bytes: &[u8]) -> Result<Vec<ImfCommand>, String> {
    if bytes.len() < 4 {
        return Err("too short for an IMF song".into());
    }
    let declared = usize::from(u16::from_le_bytes([bytes[0], bytes[1]]));
    let cmds = if declared != 0 && declared % 4 == 0 && declared <= bytes.len() - 2 {
        parse_imf(bytes).map_err(|e| e.to_string())?
    } else if bytes.len().is_multiple_of(4) {
        bytes
            .chunks_exact(4)
            .map(|c| ImfCommand { reg: c[0], value: c[1], delay: u16::from_le_bytes([c[2], c[3]]) })
            .collect()
    } else {
        return Err("not an IMF song (bad length)".into());
    };
    if cmds.iter().all(|c| c.delay == 0) {
        return Err("an IMF song with no timing (every delay is 0)".into());
    }
    Ok(cmds)
}

/// Converts a PCM WAV (8/16/24/32-bit integer or 32-bit float, mono or
/// stereo, 8-96 kHz) to QOA, keeping its channels and rate: compact music
/// for a mod (`obcr --convert-audio`, used by OBCEdit).
pub fn wav_to_qoa(bytes: &[u8]) -> Result<Vec<u8>, String> {
    let mut r = hound::WavReader::new(Cursor::new(bytes))
        .map_err(|e| format!("not a readable WAV ({e})"))?;
    let spec = r.spec();
    let ch = usize::from(spec.channels);
    if !(1..=2).contains(&ch) {
        return Err(format!("{ch} channels (mono or stereo)"));
    }
    if !MUSIC_RATES.contains(&spec.sample_rate) {
        return Err(format!("{} Hz is not a supported rate", spec.sample_rate));
    }
    if u64::from(r.duration()) > u64::from(MAX_MUSIC_SECONDS) * u64::from(spec.sample_rate) {
        return Err(format!("longer than {MAX_MUSIC_SECONDS} seconds"));
    }
    let pcm: Vec<i16> = match (spec.sample_format, spec.bits_per_sample) {
        (hound::SampleFormat::Float, 32) => r
            .samples::<f32>()
            .map(|s| s.map(|v| (v.clamp(-1.0, 1.0) * 32767.0).round() as i16))
            .collect::<Result<_, _>>(),
        (hound::SampleFormat::Int, b @ (8 | 16 | 24 | 32)) => {
            let shift = i32::from(b) - 16;
            r.samples::<i32>()
                .map(|s| {
                    s.map(|v| if shift >= 0 { (v >> shift) as i16 } else { (v << -shift) as i16 })
                })
                .collect::<Result<_, _>>()
        }
        (f, b) => return Err(format!("unsupported sample format {f:?} {b}-bit")),
    }
    .map_err(|e| format!("damaged WAV ({e})"))?;
    if pcm.is_empty() {
        return Err("no samples".into());
    }
    crate::qoa::encode(&pcm, ch, spec.sample_rate).map_err(|e| e.to_string())
}

/// Checks a music file by decoding it in full once; returns its length in
/// seconds (IMF: none).
pub fn check_music(name: &str, bytes: Arc<[u8]>) -> Result<Option<u32>, String> {
    let format = music_name(name)?;
    match format {
        MusicFormat::Imf => read_imf(&bytes).map(|_| None),
        MusicFormat::Vorbis | MusicFormat::Qoa => {
            let want =
                if format == MusicFormat::Vorbis { StreamKind::Vorbis } else { StreamKind::Qoa };
            if StreamKind::sniff(&bytes) != Some(want) {
                return Err(format!("{name} is not a {format:?} file"));
            }
            let s = PcmStream::open(bytes).map_err(|e| e.to_string())?;
            if !MUSIC_RATES.contains(&s.rate()) {
                return Err(format!("{} Hz is not a supported rate", s.rate()));
            }
            let rate = s.rate();
            let frames = s.verify(MAX_MUSIC_SECONDS).map_err(|e| e.to_string())?;
            Ok(Some((frames / u64::from(rate)) as u32))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_names() {
        assert_eq!(classify("walls/012.png"), Some(Ok(AssetName::Wall(12))));
        assert_eq!(classify("Sprites/450.PNG"), Some(Ok(AssetName::Sprite(450))));
        assert_eq!(classify("sounds/7.wav"), Some(Ok(AssetName::Sound(7))));
        assert_eq!(classify("music/Theme_1.ogg"), Some(Ok(AssetName::Music("Theme_1.ogg".into()))));
        assert_eq!(classify("mod.toml"), None);
        assert_eq!(classify("docs/notes.txt"), None);
        assert_eq!(classify("walls/"), None);
        for bad in [
            "walls/136.png",
            "walls/0012.png",
            "walls/12.bmp",
            "sprites/680.png",
            "sounds/99.wav",
            "sounds/100.wav",
            "sounds/7.ogg",
            "music/theme.mp3",
            "music/has space.ogg",
            "music/a/b.ogg",
            "walls/-1.png",
            "walls/x.png",
        ] {
            assert!(matches!(classify(bad), Some(Err(_))), "{bad}");
        }
    }

    #[test]
    fn index_refuses_duplicates() {
        let ix = AssetIndex::build(["walls/1.png", "music/A.ogg", "mod.toml"]).unwrap();
        assert_eq!(ix.walls[&1], "walls/1.png");
        assert_eq!(ix.music_path("a.OGG"), Some("music/A.ogg"));
        assert!(AssetIndex::build(["walls/1.png", "walls/001.png"]).is_err());
        assert!(AssetIndex::build(["music/a.ogg", "music/A.ogg"]).is_err());
        assert!(AssetIndex::build(["walls/1.jpg"]).is_err());
    }

    fn png_bytes(w: u32, h: u32, color: png::ColorType, data: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        let mut e = png::Encoder::new(&mut out, w, h);
        e.set_color(color);
        e.set_depth(png::BitDepth::Eight);
        let mut wr = e.write_header().unwrap();
        wr.write_image_data(data).unwrap();
        wr.finish().unwrap();
        out
    }

    #[test]
    fn decodes_pngs_of_each_colour_type() {
        let rgba: Vec<u8> = (0..64 * 64).flat_map(|i| [i as u8, 2, 3, (i % 256) as u8]).collect();
        let img = decode_png(&png_bytes(64, 64, png::ColorType::Rgba, &rgba)).unwrap();
        assert_eq!(img.at(5, 0), [5, 2, 3, 5]);
        assert_eq!(img.at(0, 1), [64, 2, 3, 64]);
        let grey = vec![9u8; 64 * 64];
        let img = decode_png(&png_bytes(64, 64, png::ColorType::Grayscale, &grey)).unwrap();
        assert_eq!(img.at(63, 63), [9, 9, 9, 255]);
        let rgb = vec![7u8; 64 * 64 * 3];
        assert_eq!(
            decode_png(&png_bytes(64, 64, png::ColorType::Rgb, &rgb)).unwrap().at(1, 1)[3],
            255
        );
    }

    #[test]
    fn refuses_bad_pngs() {
        assert!(decode_png(&png_bytes(32, 64, png::ColorType::Grayscale, &[0; 32 * 64])).is_err());
        assert!(decode_png(b"\x89PNG\r\n\x1a\nnonsense").is_err());
        assert!(decode_png(&[]).is_err());
        let good = png_bytes(64, 64, png::ColorType::Grayscale, &[0; 64 * 64]);
        assert!(decode_png(&good[..good.len() / 2]).is_err());
        assert!(decode_png(&vec![0; MAX_IMAGE_BYTES + 1]).is_err());
    }

    fn wav(spec: hound::WavSpec, samples: &[i32]) -> Vec<u8> {
        let mut out = Cursor::new(Vec::new());
        let mut w = hound::WavWriter::new(&mut out, spec).unwrap();
        for &s in samples {
            match spec.bits_per_sample {
                8 => w.write_sample(s as i8).unwrap(),
                16 => w.write_sample(s as i16).unwrap(),
                _ => w.write_sample(s).unwrap(),
            }
        }
        w.finalize().unwrap();
        out.into_inner()
    }

    #[test]
    fn decodes_wavs_to_unsigned_mono() {
        let spec = |ch, bits| hound::WavSpec {
            channels: ch,
            sample_rate: 11_025,
            bits_per_sample: bits,
            sample_format: hound::SampleFormat::Int,
        };
        let s = decode_wav(&wav(spec(1, 16), &[0, 32767, -32768])).unwrap();
        assert_eq!((s.rate, s.samples), (11_025, vec![128, 255, 0]));
        // Stereo is averaged.
        let s = decode_wav(&wav(spec(2, 16), &[32767, -32767, 32767, 32767])).unwrap();
        assert_eq!(s.samples, vec![128, 255]);
        let s = decode_wav(&wav(spec(1, 8), &[0, 127])).unwrap();
        assert_eq!(s.samples, vec![128, 255]);
    }

    #[test]
    fn refuses_bad_wavs() {
        let spec = |ch, rate| hound::WavSpec {
            channels: ch,
            sample_rate: rate,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        assert!(decode_wav(&wav(spec(3, 11_025), &[0, 0, 0])).is_err());
        assert!(decode_wav(&wav(spec(1, 96_000), &[0])).is_err());
        assert!(decode_wav(&wav(spec(1, 11_025), &[])).is_err());
        assert!(decode_wav(b"RIFF\x10\x00\x00\x00WAVEjunk").is_err());
        let long = vec![0i32; 4_000 * 31];
        assert!(decode_wav(&wav(spec(1, 4_000), &long)).is_err());
    }

    #[test]
    fn reads_both_imf_types() {
        let cmds = [0x20u8, 1, 5, 0, 0x40, 2, 0, 0];
        let mut t1 = vec![8, 0];
        t1.extend_from_slice(&cmds);
        assert_eq!(read_imf(&t1).unwrap().len(), 2);
        assert_eq!(read_imf(&cmds).unwrap()[0].delay, 5);
        assert!(read_imf(&[1, 2, 3]).is_err());
        assert!(read_imf(&[0x20, 1, 0, 0]).is_err(), "no timing");
    }

    const TONE: &[u8] = include_bytes!("../tests/data/tone.ogg");

    #[test]
    fn streams_and_checks_vorbis_music() {
        assert_eq!(check_music("tone.ogg", Arc::from(TONE)), Ok(Some(2)));
        assert!(check_music("tone.qoa", Arc::from(TONE)).is_err(), "named .qoa but Vorbis");
        let mut s = PcmStream::open(Arc::from(TONE)).unwrap();
        assert_eq!((s.kind(), s.channels(), s.rate()), (StreamKind::Vorbis, 2, 22_050));
        let mut block = Vec::new();
        let mut first = 0;
        while s.next_block(&mut block) {
            first += block.len();
        }
        s.rewind();
        let mut again = 0;
        while s.next_block(&mut block) {
            again += block.len();
        }
        assert_eq!(first, again, "rewind plays the whole file again");
        assert!(first / 2 >= 44_000, "about two seconds of stereo: {first}");
        // Truncated: decodes what is there, then ends; a check of a file cut
        // in its headers fails.
        assert!(check_music("t.ogg", Arc::from(&TONE[..200])).is_err());
    }

    #[test]
    fn converts_wav_music_to_qoa() {
        let spec = hound::WavSpec {
            channels: 2,
            sample_rate: 22_050,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let pcm: Vec<i32> = (0..22_050 * 2).map(|i| (i % 400) * 100 - 20_000).collect();
        let qoa = wav_to_qoa(&wav(spec, &pcm)).unwrap();
        let (info, out) = crate::qoa::decode(&qoa).unwrap();
        assert_eq!((info.channels, info.rate, info.samples), (2, 22_050, 22_050));
        assert_eq!(out.len(), pcm.len());
        assert_eq!(check_music("x.qoa", Arc::from(qoa)), Ok(Some(1)));
        assert!(wav_to_qoa(b"not a wav").is_err());
    }

    #[test]
    fn checks_qoa_music() {
        let pcm: Vec<i16> = (0..44_100).map(|i| ((i % 100) * 300) as i16).collect();
        let qoa: Arc<[u8]> = crate::qoa::encode(&pcm, 1, 22_050).unwrap().into();
        assert_eq!(check_music("a.qoa", Arc::clone(&qoa)), Ok(Some(2)));
        assert!(check_music("a.ogg", Arc::clone(&qoa)).is_err(), "named .ogg but QOA");
        let cut: Arc<[u8]> = qoa[..qoa.len() / 2].into();
        assert!(check_music("a.qoa", cut).is_err(), "truncated");
        assert!(check_music("a.ogg", Arc::from(&b"OggS\0\0garbage"[..])).is_err());
    }
}
