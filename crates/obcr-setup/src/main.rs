//! Self-extracting launcher for OBCR Setup.
//!
//! `cargo xtask pack-installer` appends the deployed Qt installer folder
//! (Setup, the Qt runtime, plugins) to this executable as an archive:
//!
//! ```text
//! entry*  : u16 path length, path (UTF-8, '/' separated), u32 mode,
//!           u64 size, u64 packed size, raw-deflate data
//! trailer : u64 archive length, b"OBCRSFX2"
//! ```
//!
//! At start the archive is unpacked into a fresh temporary folder, Setup runs
//! from there with our arguments, and the folder is removed when it exits.

#![cfg_attr(windows, windows_subsystem = "windows")]

use std::fs::{self, File};
use std::io::{self, BufReader, Read, Seek, SeekFrom};
use std::path::{Component, Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

const MAGIC: &[u8; 8] = b"OBCRSFX2";
const SETUP: &str = if cfg!(windows) { "OBCR-Setup.exe" } else { "OBCR-Setup" };

fn main() {
    let code = match run() {
        Ok(code) => code,
        Err(e) => {
            report(&format!("OBCR Setup could not start: {e}"));
            1
        }
    };
    std::process::exit(code);
}

fn run() -> io::Result<i32> {
    let exe = std::env::current_exe()?;
    let dir = unpack(&exe)?;
    let status = Command::new(dir.join(SETUP))
        .args(std::env::args_os().skip(1))
        .current_dir(std::env::temp_dir())
        .env("OBCR_SETUP_PACKAGE", &exe)
        .status();
    let _ = fs::remove_dir_all(&dir);
    Ok(status?.code().unwrap_or(1))
}

/// Unpacks the appended archive into a new temporary folder.
fn unpack(exe: &Path) -> io::Result<PathBuf> {
    let bad = |msg: &str| io::Error::new(io::ErrorKind::InvalidData, msg.to_string());
    let mut f = File::open(exe)?;
    let len = f.metadata()?.len();
    if len < 16 {
        return Err(bad("no installer inside this file"));
    }
    f.seek(SeekFrom::Start(len - 16))?;
    let mut trailer = [0u8; 16];
    f.read_exact(&mut trailer)?;
    if &trailer[8..] != MAGIC {
        return Err(bad("no installer inside this file"));
    }
    let archive = u64::from_le_bytes(trailer[..8].try_into().unwrap_or_default());
    let start = (len - 16).checked_sub(archive).ok_or_else(|| bad("damaged installer"))?;
    f.seek(SeekFrom::Start(start))?;
    let mut r = BufReader::new(f).take(archive);

    let stamp = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_nanos());
    let dir = std::env::temp_dir().join(format!("obcr-setup-{}-{stamp}", std::process::id()));
    fs::create_dir_all(&dir)?;
    loop {
        let mut n = [0u8; 2];
        match r.read_exact(&mut n) {
            Ok(()) => {}
            Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => break,
            Err(e) => return Err(e),
        }
        let mut name = vec![0u8; usize::from(u16::from_le_bytes(n))];
        r.read_exact(&mut name)?;
        let name = String::from_utf8(name).map_err(|_| bad("damaged installer"))?;
        let rel = Path::new(&name);
        // Only plain relative paths.
        if !rel.components().all(|c| matches!(c, Component::Normal(_))) {
            return Err(bad("damaged installer"));
        }
        let mut header = [0u8; 20];
        r.read_exact(&mut header)?;
        let mode = u32::from_le_bytes(header[..4].try_into().unwrap_or_default());
        let size = u64::from_le_bytes(header[4..12].try_into().unwrap_or_default());
        let packed = u64::from_le_bytes(header[12..].try_into().unwrap_or_default());
        let mut data = vec![0u8; usize::try_from(packed).map_err(|_| bad("damaged installer"))?];
        r.read_exact(&mut data)?;
        let data =
            miniz_oxide::inflate::decompress_to_vec(&data).map_err(|_| bad("damaged installer"))?;
        if data.len() as u64 != size {
            return Err(bad("damaged installer"));
        }
        let path = dir.join(rel);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&path, &data)?;
        set_mode(&path, mode)?;
    }
    Ok(dir)
}

#[cfg(unix)]
fn set_mode(path: &Path, mode: u32) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(mode & 0o777))
}

#[cfg(not(unix))]
fn set_mode(_: &Path, _: u32) -> io::Result<()> {
    Ok(())
}

/// Shows an error: a message box on Windows (there is no console), standard
/// error elsewhere.
#[cfg(windows)]
#[allow(unsafe_code)] // One Win32 call with valid, NUL-terminated wide strings.
fn report(msg: &str) {
    #[link(name = "user32")]
    unsafe extern "system" {
        fn MessageBoxW(hwnd: isize, text: *const u16, caption: *const u16, kind: u32) -> i32;
    }
    let wide = |s: &str| s.encode_utf16().chain(std::iter::once(0)).collect::<Vec<u16>>();
    let (text, caption) = (wide(msg), wide("OBCR Setup"));
    // SAFETY: both pointers point to live NUL-terminated UTF-16 buffers;
    // MB_ICONERROR = 0x10.
    unsafe {
        MessageBoxW(0, text.as_ptr(), caption.as_ptr(), 0x10);
    }
}

#[cfg(not(windows))]
fn report(msg: &str) {
    eprintln!("{msg}");
}
