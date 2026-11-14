//! The installer's helpers (`--check-data`, `--extract-data`): identify a
//! copy of the game, in a folder or on a CD image, and copy what OBCR needs
//! into a data folder, ripping the CD music (ADR-022). Output is one
//! tab-separated record per line for `installer/` to parse.

use std::io::Write;
use std::path::{Path, PathBuf};

use obc_formats::DataDir;
use obc_formats::disc::Disc;
use obc_formats::release::{self, FileStatus, Identification, Role};

/// Where the game files come from.
enum Source {
    Dir(DataDir),
    Disc(Box<Disc>),
}

impl Source {
    fn open(path: &Path) -> Result<Self, String> {
        if Disc::is_image(path) {
            return Disc::open(path).map(|d| Self::Disc(Box::new(d))).map_err(|e| e.to_string());
        }
        DataDir::open(path).map(Self::Dir).map_err(|e| e.to_string())
    }

    fn read(&self, name: &str) -> Option<Vec<u8>> {
        match self {
            Self::Dir(d) => d.read(name).ok(),
            Self::Disc(d) => d.read(name).ok(),
        }
    }

    fn path(&self, name: &str) -> String {
        match self {
            Self::Dir(d) => d.path(name).map(|p| p.display().to_string()).unwrap_or_default(),
            Self::Disc(d) => {
                if d.size(name).is_some() {
                    format!("{}|{}", d.source().display(), name.to_ascii_uppercase())
                } else {
                    String::new()
                }
            }
        }
    }

    fn identify(&self) -> Identification {
        release::identify_with(|name| self.read(name))
    }
}

fn role_name(r: Role) -> &'static str {
    match r {
        Role::Required => "required",
        Role::Sequence => "sequence",
        Role::UserState => "user",
        Role::Extra => "extra",
    }
}

/// QOA bytes for `samples` stereo samples: 2 × 3.2 bits each, plus frame headers.
fn qoa_estimate(samples: u64) -> u64 {
    samples * 8 / 10 + samples / 5120 * 80 + 8
}

/// `--check-data DIR|IMAGE`: prints `release`, `source`, one `file` line per
/// known file (role, status, name, path), one `audio` line per CD track
/// (number, seconds, estimated installed bytes), then `complete yes|no`.
/// Exit code 0 when every required file matches, 2 when not, 1 on error.
pub fn check_data(path: &Path) -> i32 {
    let src = match Source::open(path) {
        Ok(s) => s,
        Err(e) => {
            println!("error\t{e}");
            return 1;
        }
    };
    let id = src.identify();
    println!("release\t{}\t{}", id.release.id, id.release.description);
    println!("source\t{}", if matches!(src, Source::Disc(_)) { "disc" } else { "folder" });
    for c in &id.checks {
        let status = match c.status {
            FileStatus::Ok => "ok",
            FileStatus::Missing => "missing",
            FileStatus::Present => "present",
            FileStatus::Mismatch { .. } => "mismatch",
        };
        let size = match &c.status {
            FileStatus::Ok => c.file.size,
            _ => 0,
        };
        println!(
            "file\t{}\t{status}\t{}\t{}\t{size}",
            role_name(c.file.role),
            c.file.name,
            src.path(c.file.name)
        );
    }
    if let Source::Disc(d) = &src {
        for t in d.audio_tracks() {
            println!("audio\t{}\t{:.1}\t{}", t.number, t.seconds(), qoa_estimate(t.samples()));
        }
    }
    let complete = id.is_complete();
    println!("complete\t{}", if complete { "yes" } else { "no" });
    if complete { 0 } else { 2 }
}

fn progress(pct: f64, msg: &str) {
    println!("progress\t{}\t{msg}", pct.clamp(0.0, 100.0) as u32);
    std::io::stdout().flush().ok();
}

/// `--extract-data SOURCE DIR`: copies the verified required files and
/// sequences into `DIR` under their canonical names; from a CD image also
/// rips the music tracks to `TRACK02.QOA`... Prints `progress` lines
/// (percent, message), a `wrote` line per file created, then `done`.
/// Exit code 0 on success, 2 when the game files are incomplete, 1 on error.
pub fn extract_data(from: &Path, to: &Path) -> i32 {
    match extract(from, to) {
        Ok(code) => code,
        Err(e) => {
            println!("error\t{e}");
            1
        }
    }
}

fn extract(from: &Path, to: &Path) -> Result<i32, String> {
    let src = Source::open(from)?;
    progress(0.0, "Checking the game files");
    let id = src.identify();
    if !id.is_complete() {
        println!("error\tThe game files are incomplete or a different version.");
        return Ok(2);
    }
    std::fs::create_dir_all(to).map_err(|e| format!("Cannot create {}: {e}", to.display()))?;
    let files: Vec<_> = id
        .checks
        .iter()
        .filter(|c| {
            matches!(c.file.role, Role::Required | Role::Sequence) && c.status == FileStatus::Ok
        })
        .map(|c| c.file)
        .collect();
    let tracks = match &src {
        Source::Disc(d) if id.release.cd => d.audio_tracks().to_vec(),
        _ => Vec::new(),
    };
    // Files take the first 15 %, the music the rest (encoding dominates).
    let file_share = if tracks.is_empty() { 100.0 } else { 15.0 };
    let total_bytes: u64 = files.iter().map(|f| f.size).sum::<u64>().max(1);
    let mut done_bytes = 0;
    for f in &files {
        let verb = if matches!(src, Source::Disc(_)) { "Extracting" } else { "Copying" };
        progress(
            file_share * done_bytes as f64 / total_bytes as f64,
            &format!("{verb} {}", f.name),
        );
        let data = src.read(f.name).ok_or_else(|| format!("Cannot read {}", f.name))?;
        write_file(&to.join(f.name), &data)?;
        done_bytes += f.size;
    }
    if let Source::Disc(disc) = &src {
        let total: u64 = tracks.iter().map(|t| t.samples()).sum::<u64>().max(1);
        let mut before = 0u64;
        for (i, t) in tracks.iter().enumerate() {
            // Disc track 2 is game track 1 (CD_RELEASE.md §4).
            let name = format!("TRACK{:02}.QOA", t.number);
            let msg = format!("Ripping music track {} of {}", i + 1, tracks.len());
            let path = to.join(&name);
            let tmp = to.join(format!("{name}.part"));
            let mut out = std::io::BufWriter::new(
                std::fs::File::create(&tmp)
                    .map_err(|e| format!("Cannot create {}: {e}", tmp.display()))?,
            );
            let mut last = -1i64;
            disc.rip_qoa(t, &mut out, |f| {
                let pct = file_share
                    + (100.0 - file_share) * (before as f64 + f * t.samples() as f64)
                        / total as f64;
                if pct as i64 != last {
                    last = pct as i64;
                    progress(pct, &msg);
                }
            })
            .map_err(|e| e.to_string())?;
            out.flush().map_err(|e| e.to_string())?;
            drop(out);
            std::fs::rename(&tmp, &path).map_err(|e| format!("{}: {e}", path.display()))?;
            println!("wrote\t{}", path.display());
            before += t.samples();
        }
    }
    progress(100.0, "Done");
    println!("done");
    Ok(0)
}

fn write_file(path: &PathBuf, data: &[u8]) -> Result<(), String> {
    std::fs::write(path, data).map_err(|e| format!("Cannot write {}: {e}", path.display()))?;
    println!("wrote\t{}", path.display());
    Ok(())
}
