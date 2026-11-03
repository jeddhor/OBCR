//! Save slots (GAMEFLOW §9): slots 0–8 named by the player, slot 9 AutoSave.
//! Files live in the user data directory as `svgm<n>.obcr` / `autsv.obcr`.

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use obc_sim::save::{self, FORMAT, SaveHeader};
use obc_sim::world::World;

use crate::assets::Assets;

pub const SLOTS: usize = 10;
pub const AUTOSAVE: usize = 9;
/// `US_LineInput` limit for save names.
pub const NAME_MAX: usize = 11;

pub fn slot_path(dir: &Path, slot: usize) -> PathBuf {
    if slot == AUTOSAVE { dir.join("autsv.obcr") } else { dir.join(format!("svgm{slot}.obcr")) }
}

/// Headers of every slot (`None`: empty or unreadable).
pub fn list(dir: &Path) -> Vec<Option<SaveHeader>> {
    (0..SLOTS)
        .map(|s| std::fs::read(slot_path(dir, s)).ok().and_then(|b| save::read_header(&b).ok()))
        .collect()
}

/// Writes `w` to `slot`.
pub fn write(dir: &Path, slot: usize, w: &World, name: &str, a: &Assets) -> Result<(), String> {
    let header = SaveHeader {
        format: FORMAT,
        obcr: env!("CARGO_PKG_VERSION").into(),
        release: a.release.release.id.into(),
        mapon: w.gamestate.mapon,
        difficulty: w.gamestate.difficulty,
        name: name.into(),
        timestamp: SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs()),
    };
    let bytes = save::write(w, &header).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let path = slot_path(dir, slot);
    // Write then rename, so a failed save never destroys the previous one.
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, bytes).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, &path).map_err(|e| e.to_string())
}

/// Reads `slot`.
pub fn read(dir: &Path, slot: usize, a: &Assets) -> Result<(SaveHeader, World), String> {
    let bytes = std::fs::read(slot_path(dir, slot)).map_err(|e| e.to_string())?;
    let (h, w) = save::read(&bytes, a.game.clone(), a.trig.clone()).map_err(|e| e.to_string())?;
    if h.release != a.release.release.id {
        return Err(format!("saved with data release {}", h.release));
    }
    Ok((h, w))
}
