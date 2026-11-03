//! Locates the original game data for data-gated tests (DESIGN §9.2).
//!
//! Tests that need the user's copy of Operation Body Count call [`require!`],
//! which returns the data directory or skips the test with a loud message.

use std::path::{Path, PathBuf};

/// Workspace root (two levels above this crate).
pub fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Data directory: `OBC_DATA_DIR`, then `.local/data/obc-full`, then `../OBC`.
pub fn data_dir() -> Option<PathBuf> {
    if let Some(dir) = std::env::var_os("OBC_DATA_DIR") {
        let dir = PathBuf::from(dir);
        return dir.is_dir().then_some(dir);
    }
    let root = workspace_root();
    [root.join(".local/data/obc-full"), root.join("../OBC")]
        .into_iter()
        .find(|dir| dir.join("gfxtiles.bc").is_file() || dir.join("GFXTILES.BC").is_file())
}

/// Returns the data directory, or prints a skip notice and returns from the test.
#[macro_export]
macro_rules! require {
    () => {
        match $crate::data_dir() {
            Some(dir) => dir,
            None => {
                eprintln!("SKIPPED: original game data not found (set OBC_DATA_DIR)");
                return;
            }
        }
    };
}
