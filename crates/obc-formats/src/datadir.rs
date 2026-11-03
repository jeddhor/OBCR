//! Finding the user's game data and opening files case-insensitively (DESIGN §6.7).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::error::{FormatError, Result};

/// File whose presence marks a directory as Operation Body Count data.
const MARKER: &str = "GFXTILES.BC";

/// A directory of original game files, indexed by upper-case file name.
#[derive(Debug, Clone)]
pub struct DataDir {
    root: PathBuf,
    files: BTreeMap<String, PathBuf>,
}

impl DataDir {
    /// Indexes the files directly inside `root`.
    pub fn open(root: impl Into<PathBuf>) -> Result<Self> {
        let root = root.into();
        let io = |source| FormatError::Io { path: root.clone(), source };
        let mut files = BTreeMap::new();
        for entry in std::fs::read_dir(&root).map_err(io)? {
            let entry = entry.map_err(io)?;
            if entry.file_type().map_err(io)?.is_file() {
                let name = entry.file_name().to_string_lossy().to_ascii_uppercase();
                files.insert(name, entry.path());
            }
        }
        Ok(Self { root, files })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Path of `name` (any case), if present.
    pub fn path(&self, name: &str) -> Option<&Path> {
        self.files.get(&name.to_ascii_uppercase()).map(PathBuf::as_path)
    }

    pub fn contains(&self, name: &str) -> bool {
        self.path(name).is_some()
    }

    /// Reads the whole file `name` (any case).
    pub fn read(&self, name: &str) -> Result<Vec<u8>> {
        let path = self.path(name).ok_or_else(|| FormatError::Missing(name.to_owned()))?;
        std::fs::read(path).map_err(|source| FormatError::Io { path: path.to_owned(), source })
    }

    /// Upper-case names of all files in the directory.
    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.files.keys().map(String::as_str)
    }
}

/// True if `dir` looks like an Operation Body Count data directory.
pub fn is_data_dir(dir: &Path) -> bool {
    DataDir::open(dir).is_ok_and(|d| d.contains(MARKER))
}

/// Finds the data directory using the resolution order of DESIGN §6.7:
/// explicit flag, `OBC_DATA_DIR`, configured path, next to the executable,
/// Steam's default library, then development locations relative to the
/// current directory.
pub fn discover(explicit: Option<&Path>, configured: Option<&Path>) -> Option<PathBuf> {
    let mut candidates: Vec<PathBuf> = Vec::new();
    candidates.extend(explicit.map(Path::to_path_buf));
    candidates.extend(std::env::var_os("OBC_DATA_DIR").map(PathBuf::from));
    candidates.extend(configured.map(Path::to_path_buf));
    if let Some(exe_dir) =
        std::env::current_exe().ok().and_then(|p| p.parent().map(Path::to_path_buf))
    {
        candidates.extend(["OBC", "data", ""].iter().map(|sub| exe_dir.join(sub)));
    }
    candidates.extend(steam_candidates());
    candidates.extend([".local/data/obc-full", "../OBC", "OBC", "data"].iter().map(PathBuf::from));
    candidates.into_iter().find(|dir| is_data_dir(dir))
}

/// Default Steam install locations of the Steam release (app 1627120, data in `C/BCCD`).
fn steam_candidates() -> Vec<PathBuf> {
    const GAME: &str = "steamapps/common/Operation Body Count/C/BCCD";
    let mut roots = vec![
        PathBuf::from("C:/Program Files (x86)/Steam"),
        PathBuf::from("C:/Program Files/Steam"),
    ];
    if let Some(home) = std::env::var_os("HOME").map(PathBuf::from) {
        roots.push(home.join(".steam/steam"));
        roots.push(home.join(".local/share/Steam"));
        roots.push(home.join("Library/Application Support/Steam"));
    }
    roots.into_iter().map(|r| r.join(GAME)).collect()
}
