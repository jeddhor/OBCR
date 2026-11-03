//! `cargo xtask <command>`: project automation (AGENT_GUIDE §2.3).
//!
//! - `ci`: format check, clippy with warnings denied, tests, license policy.
//! - `lint`: crate dependency rules and a scan for committed original data.
//! - `test-data`: tests including those that need the original data.

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail};

fn main() -> Result<()> {
    let task = std::env::args().nth(1).unwrap_or_default();
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    std::env::set_current_dir(&root)?;
    match task.as_str() {
        "ci" => ci(),
        "lint" => lint(&root),
        "test-data" => test_data(&root),
        _ => {
            eprintln!("usage: cargo xtask <ci|lint|test-data>");
            bail!("unknown task {task:?}")
        }
    }
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
    run_env("cargo", &["test", "--workspace", "--", "--include-ignored"], &[("OBC_DATA_DIR", &dir)])
}

/// Allowed internal dependencies per crate (DESIGN §6.3).
const RULES: &[(&str, &[&str])] = &[
    ("obc-formats", &[]),
    ("obc-core", &[]),
    ("obc-sim", &["obc-core", "obc-formats"]),
    ("obc-render", &["obc-core", "obc-formats", "obc-sim"]),
    ("obc-audio", &["obc-core", "obc-formats"]),
    ("obc-ui", &["obc-core", "obc-formats", "obc-sim", "obc-render", "obc-audio"]),
    ("obc-net", &["obc-core", "obc-sim"]),
    ("obc-platform", &[]),
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
