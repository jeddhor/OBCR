//! Save slots (GAMEFLOW §9): slots 0–8 named by the player, slot 9 AutoSave.
//! Files live in the user data directory as `svgm<n>.obcr` / `autsv.obcr`.

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use obc_sim::save::{self, FORMAT, SaveHeader};
use obc_sim::world::World;

use crate::assets::Assets;
use crate::screen::Ctx;

pub const SLOTS: usize = 10;
pub const AUTOSAVE: usize = 9;
/// `US_LineInput` limit for save names.
pub const NAME_MAX: usize = 11;

pub fn slot_path(dir: &Path, slot: usize) -> PathBuf {
    if slot == AUTOSAVE { dir.join("autsv.obcr") } else { dir.join(format!("svgm{slot}.obcr")) }
}

/// The save folder of the active campaign: the saves folder for the
/// original game, `saves/<mod id>/` for a mod, so each mod has its own slots.
pub fn dir_for(ctx: &Ctx) -> PathBuf {
    let id = crate::campaign::active_id(ctx);
    if id.is_empty() { ctx.save_dir.to_owned() } else { ctx.save_dir.join(id) }
}

/// Headers of every slot of the active campaign (`None`: empty or unreadable).
pub fn list(ctx: &Ctx) -> Vec<Option<SaveHeader>> {
    let dir = dir_for(ctx);
    (0..SLOTS)
        .map(|s| std::fs::read(slot_path(&dir, s)).ok().and_then(|b| save::read_header(&b).ok()))
        .collect()
}

/// Writes `w` to `slot` of the active campaign.
pub fn write(ctx: &Ctx, slot: usize, w: &World, name: &str) -> Result<(), String> {
    let a: &Assets = ctx.assets;
    let header = SaveHeader {
        format: FORMAT,
        obcr: env!("CARGO_PKG_VERSION").into(),
        release: a.release.release.id.into(),
        mapon: w.gamestate.mapon,
        difficulty: w.gamestate.difficulty,
        name: name.into(),
        timestamp: SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs()),
    };
    let (mod_id, mod_version) = crate::campaign::active_meta(ctx);
    let bytes = save::write_mod(w, &header, &mod_id, &mod_version).map_err(|e| e.to_string())?;
    let dir = dir_for(ctx);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let path = slot_path(&dir, slot);
    // Write then rename, so a failed save never destroys the previous one.
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, bytes).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, &path).map_err(|e| e.to_string())
}

/// Reads `slot` of the active campaign.
pub fn read(ctx: &Ctx, slot: usize) -> Result<(SaveHeader, World), String> {
    let a: &Assets = ctx.assets;
    let bytes = std::fs::read(slot_path(&dir_for(ctx), slot)).map_err(|e| e.to_string())?;
    let (_, mod_id, _) = save::read_mod(&bytes).map_err(|e| e.to_string())?;
    if mod_id != crate::campaign::active_id(ctx) {
        return Err(if mod_id.is_empty() {
            "saved in the original game".into()
        } else {
            format!("saved in the mod {mod_id}")
        });
    }
    let (h, w) = save::read(&bytes, a.game.clone(), a.trig.clone()).map_err(|e| e.to_string())?;
    if h.release != a.release.release.id {
        return Err(format!("saved with data release {}", h.release));
    }
    Ok((h, w))
}
