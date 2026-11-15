//! `cargo xtask <command>`: project automation (AGENT_GUIDE §2.3).
//!
//! - `ci`: format check, clippy with warnings denied, tests, license policy.
//! - `lint`: crate dependency rules and a scan for committed original data.
//! - `test-data`: tests including those that need the original data.
//! - `pack-installer <launcher> <dir> <out>`: appends the deployed Setup
//!   folder to the `obcr-setup` launcher, giving one self-extracting file
//!   (format in `crates/obcr-setup/src/main.rs`).

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail};

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let task = args.first().cloned().unwrap_or_default();
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    // Paths given to pack-installer are relative to where it was started.
    let cwd = std::env::current_dir()?;
    std::env::set_current_dir(&root)?;
    match task.as_str() {
        "ci" => ci(),
        "lint" => lint(&root),
        "test-data" => test_data(&root),
        "pack-installer" if args.len() == 4 => {
            pack_installer(&cwd.join(&args[1]), &cwd.join(&args[2]), &cwd.join(&args[3]))
        }
        _ => {
            eprintln!("usage: cargo xtask <ci|lint|test-data|pack-installer LAUNCHER DIR OUT>");
            bail!("unknown task {task:?}")
        }
    }
}

/// The mode bits that mark a symbolic link in the payload (`S_IFLNK`).
const LINK_MODE: u32 = 0o120_777;

/// Writes `out` = `launcher` + an archive of every file under `dir`, each
/// deflate-compressed, + trailer (archive length, `OBCRSFX3`).
///
/// A symbolic link is packed as a link (mode [`LINK_MODE`], data = its
/// target), not as a second copy of what it points to: PyInstaller's Linux
/// freeze of OBCEdit links its biggest libraries into place, and following
/// them packed each twice. Only relative links to a file inside `dir` are
/// accepted.
fn pack_installer(launcher: &Path, dir: &Path, out: &Path) -> Result<()> {
    fn walk(dir: &Path, base: &Path, files: &mut Vec<PathBuf>) -> Result<()> {
        let mut entries: Vec<_> = std::fs::read_dir(dir)?.collect::<std::io::Result<_>>()?;
        entries.sort_by_key(|e| e.file_name());
        for e in entries {
            let p = e.path();
            // `file_type` does not follow links: a link to a folder is packed
            // as a link (and refused below), never walked.
            if e.file_type()?.is_dir() {
                walk(&p, base, files)?;
            } else {
                files.push(p.strip_prefix(base)?.to_path_buf());
            }
        }
        Ok(())
    }
    let mut files = Vec::new();
    walk(dir, dir, &mut files)?;
    let mut archive = Vec::new();
    let mut links = 0;
    for rel in &files {
        let name =
            rel.components().map(|c| c.as_os_str().to_string_lossy()).collect::<Vec<_>>().join("/");
        let path = dir.join(rel);
        let (data, mode) = if std::fs::symlink_metadata(&path)?.file_type().is_symlink() {
            let target = link_target(dir, rel)?;
            links += 1;
            (target.into_bytes(), LINK_MODE)
        } else {
            let data = std::fs::read(&path).with_context(|| path.display().to_string())?;
            #[cfg(unix)]
            let mode = {
                use std::os::unix::fs::PermissionsExt;
                std::fs::metadata(&path)?.permissions().mode() & 0o777
            };
            #[cfg(not(unix))]
            let mode = 0o644u32;
            (data, mode)
        };
        let packed = miniz_oxide::deflate::compress_to_vec(&data, 9);
        archive.extend_from_slice(&u16::try_from(name.len())?.to_le_bytes());
        archive.extend_from_slice(name.as_bytes());
        archive.extend_from_slice(&mode.to_le_bytes());
        archive.extend_from_slice(&(data.len() as u64).to_le_bytes());
        archive.extend_from_slice(&(packed.len() as u64).to_le_bytes());
        archive.extend_from_slice(&packed);
    }
    let mut f = std::fs::File::create(out).with_context(|| out.display().to_string())?;
    f.write_all(&std::fs::read(launcher).with_context(|| launcher.display().to_string())?)?;
    f.write_all(&archive)?;
    f.write_all(&(archive.len() as u64).to_le_bytes())?;
    f.write_all(b"OBCRSFX3")?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(out, std::fs::Permissions::from_mode(0o755))?;
    }
    eprintln!(
        "packed {} files ({links} links, {} bytes) into {}",
        files.len(),
        archive.len(),
        out.display()
    );
    Ok(())
}

/// The target of the link `dir/rel`, '/'-separated, if it is relative and
/// names a file inside `dir`.
fn link_target(dir: &Path, rel: &Path) -> Result<String> {
    use std::path::Component;
    let path = dir.join(rel);
    let target = std::fs::read_link(&path).with_context(|| path.display().to_string())?;
    // Resolve it against the link's folder without leaving `dir`.
    let mut resolved: Vec<_> = rel.parent().map(|p| p.components().collect()).unwrap_or_default();
    for c in target.components() {
        match c {
            Component::Normal(_) => resolved.push(c),
            Component::CurDir => {}
            Component::ParentDir if !resolved.is_empty() => {
                resolved.pop();
            }
            _ => bail!("{}: link to {} leaves the folder", path.display(), target.display()),
        }
    }
    let inside: PathBuf = resolved.iter().collect();
    if !dir.join(&inside).is_file() {
        bail!("{}: link to {} does not name a file", path.display(), target.display());
    }
    Ok(target.components().map(|c| c.as_os_str().to_string_lossy()).collect::<Vec<_>>().join("/"))
}

fn run(program: &str, args: &[&str]) -> Result<()> {
    run_env(program, args, &[])
}

fn run_env(program: &str, args: &[&str], envs: &[(&str, &Path)]) -> Result<()> {
    eprintln!("$ {program} {}", args.join(" "));
    let status = Command::new(program)
        .args(args)
        .envs(envs.iter().map(|(k, v)| (*k, *v)))
        .status()
        .with_context(|| program.to_owned())?;
    if !status.success() {
        bail!("{program} {} failed", args.join(" "));
    }
    Ok(())
}

fn ci() -> Result<()> {
    run("cargo", &["fmt", "--all", "--check"])?;
    run("cargo", &["clippy", "--workspace", "--all-targets", "--", "-D", "warnings"])?;
    run("cargo", &["test", "--workspace"])?;
    lint(Path::new("."))?;
    if Command::new("cargo").args(["deny", "--version"]).output().is_ok_and(|o| o.status.success())
    {
        run("cargo", &["deny", "check", "licenses", "bans"])?;
    } else {
        eprintln!("note: cargo-deny not installed; license check skipped");
    }
    Ok(())
}

fn test_data(root: &Path) -> Result<()> {
    let dir = match std::env::var_os("OBC_DATA_DIR") {
        Some(d) => PathBuf::from(d),
        None if root.join(".local/data/obc-full").is_dir() => root.join(".local/data/obc-full"),
        None => root.join("../OBC"),
    };
    eprintln!("OBC_DATA_DIR={}", dir.display());
    run_env(
        "cargo",
        &["test", "--workspace", "--", "--include-ignored"],
        &[("OBC_DATA_DIR", &dir)],
    )?;
    // OBCEdit's catalog must match what the engine's code says (MODS_AND_OBCEDIT.md §5.4).
    let catalog = root.join("obcedit/resources/catalog.json");
    let data = dir.to_string_lossy().into_owned();
    let cat = catalog.to_string_lossy().into_owned();
    run(
        "cargo",
        &["run", "-q", "-p", "obc-tool", "--", "--data", &data, "catalog", "--verify", &cat],
    )
}

/// Allowed internal dependencies per crate (DESIGN §6.3).
const RULES: &[(&str, &[&str])] = &[
    ("obc-formats", &[]),
    ("obc-core", &[]),
    ("obc-sim", &["obc-core", "obc-formats"]),
    ("obc-render", &["obc-core", "obc-formats", "obc-sim"]),
    ("obc-audio", &["obc-core", "obc-formats"]),
    ("obc-ui", &["obc-core", "obc-formats", "obc-sim", "obc-render", "obc-audio", "obc-net"]),
    ("obc-net", &[]),
    ("obc-sim", &["obc-core", "obc-formats"]),
    ("obc-platform", &["obc-core"]),
    ("obcr-setup", &[]),
];

fn lint(root: &Path) -> Result<()> {
    let mut problems = Vec::new();
    for (krate, allowed) in RULES {
        let manifest = std::fs::read_to_string(root.join("crates").join(krate).join("Cargo.toml"))?;
        let deps =
            manifest.split("[dependencies]").nth(1).unwrap_or("").split("\n[").next().unwrap_or("");
        for line in deps.lines() {
            let name = line.split(['.', '=', ' ']).next().unwrap_or("").trim();
            if name.starts_with("obc-") && !allowed.contains(&name) {
                problems.push(format!("{krate} must not depend on {name} (DESIGN §6.3)"));
            }
        }
    }
    for file in tracked_files()? {
        let Ok(bytes) = std::fs::read(&file) else { continue };
        for (sig, what) in
            [(&b"TED5v1.0"[..], "TED5 map data"), (&b"!ID!"[..], "id audio/map data")]
        {
            if bytes.windows(sig.len()).any(|w| w == sig) && !is_text_doc(&file) {
                problems.push(format!("{} contains {what}", file.display()));
            }
        }
        if bytes.starts_with(b"MZ") {
            problems.push(format!("{} is a DOS/Windows executable", file.display()));
        }
    }
    if problems.is_empty() {
        eprintln!("lint: ok");
        Ok(())
    } else {
        for p in &problems {
            eprintln!("lint: {p}");
        }
        bail!("{} lint problem(s)", problems.len())
    }
}

/// Source and docs mention the signatures in prose; only flag non-text files.
fn is_text_doc(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|e| e.to_str()),
        Some("md" | "rs" | "toml" | "java" | "py" | "txt")
    )
}

fn tracked_files() -> Result<Vec<PathBuf>> {
    let out = Command::new("git").args(["ls-files", "-z"]).output().context("git ls-files")?;
    Ok(out
        .stdout
        .split(|&b| b == 0)
        .filter(|s| !s.is_empty())
        .map(|s| PathBuf::from(String::from_utf8_lossy(s).into_owned()))
        .collect())
}
