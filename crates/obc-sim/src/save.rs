//! OBCR save files (DESIGN §7.7, M11): a magic, a postcard header and a
//! postcard-serialized [`World`]. Not the original `.SAV` layout.
//!
//! ```text
//! format 4: "OBCRSAVE"  u32 header length  header  u32 extra length  extra  world
//! format 3: "OBCRSAVE"  u32 header length  header                          world
//! ```
//!
//! The extra block (format 4, M17) carries the floor's [`LevelProfile`] and
//! the mod the game belongs to. Format-3 saves still load: they are original
//! games, and their profile is the stock one of their floor.

use std::sync::Arc;

use obc_core::trig::Tables;
use serde::{Deserialize, Serialize};

use crate::data::GameData;
use crate::level::LevelProfile;
use crate::world::World;

pub const MAGIC: &[u8; 8] = b"OBCRSAVE";
/// Bumped whenever `World` or the header changes shape.
pub const FORMAT: u32 = 4;
/// The oldest format still read (format 3: no extra block).
pub const OLDEST_FORMAT: u32 = 3;

/// What format 4 adds after the header.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct SaveExtra {
    pub level: LevelProfile,
    /// The mod's id, empty for the original game.
    pub mod_id: String,
    pub mod_version: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SaveHeader {
    pub format: u32,
    /// OBCR version that wrote the file.
    pub obcr: String,
    /// Data release id (`OBC-A`).
    pub release: String,
    pub mapon: i16,
    pub difficulty: i16,
    /// Slot name typed by the player.
    pub name: String,
    /// Seconds since the Unix epoch.
    pub timestamp: u64,
}

#[derive(Debug, thiserror::Error)]
pub enum SaveError {
    #[error("not an OBCR save file")]
    BadMagic,
    #[error("save format {0} is not supported (expected {OLDEST_FORMAT} to {FORMAT})")]
    Format(u32),
    #[error("corrupt save: {0}")]
    Corrupt(#[from] postcard::Error),
}

/// Serializes `w` with `header`, as an original-game save.
pub fn write(w: &World, header: &SaveHeader) -> Result<Vec<u8>, SaveError> {
    write_mod(w, header, "", "")
}

/// Serializes `w` with `header`, belonging to mod `mod_id` (empty: the original game).
pub fn write_mod(
    w: &World,
    header: &SaveHeader,
    mod_id: &str,
    mod_version: &str,
) -> Result<Vec<u8>, SaveError> {
    let mut header = header.clone();
    header.format = FORMAT;
    let head = postcard::to_stdvec(&header)?;
    let extra = postcard::to_stdvec(&SaveExtra {
        level: w.level.clone(),
        mod_id: mod_id.to_owned(),
        mod_version: mod_version.to_owned(),
    })?;
    let mut out = Vec::with_capacity(64 * 1024);
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&(head.len() as u32).to_le_bytes());
    out.extend_from_slice(&head);
    out.extend_from_slice(&(extra.len() as u32).to_le_bytes());
    out.extend_from_slice(&extra);
    out.extend_from_slice(&postcard::to_stdvec(w)?);
    Ok(out)
}

fn u32_at(bytes: &[u8], at: usize) -> Result<usize, SaveError> {
    let b = bytes.get(at..at + 4).ok_or(SaveError::BadMagic)?;
    Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]) as usize)
}

/// Header, extra block (`None` for format 3) and the world's bytes.
fn split(bytes: &[u8]) -> Result<(SaveHeader, Option<SaveExtra>, &[u8]), SaveError> {
    if bytes.len() < 12 || &bytes[..8] != MAGIC {
        return Err(SaveError::BadMagic);
    }
    let len = u32_at(bytes, 8)?;
    let head = bytes.get(12..12 + len).ok_or(SaveError::BadMagic)?;
    let header: SaveHeader = postcard::from_bytes(head)?;
    if !(OLDEST_FORMAT..=FORMAT).contains(&header.format) {
        return Err(SaveError::Format(header.format));
    }
    let rest = &bytes[12 + len..];
    if header.format == 3 {
        return Ok((header, None, rest));
    }
    let elen = u32_at(rest, 0)?;
    let extra: SaveExtra = postcard::from_bytes(rest.get(4..4 + elen).ok_or(SaveError::BadMagic)?)?;
    Ok((header, Some(extra), &rest[4 + elen..]))
}

/// Reads only the header (for slot lists).
pub fn read_header(bytes: &[u8]) -> Result<SaveHeader, SaveError> {
    split(bytes).map(|(h, _, _)| h)
}

/// Reads the header and the mod the save belongs to (empty: the original game).
pub fn read_mod(bytes: &[u8]) -> Result<(SaveHeader, String, String), SaveError> {
    let (h, extra, _) = split(bytes)?;
    let extra = extra.unwrap_or_default();
    Ok((h, extra.mod_id, extra.mod_version))
}

/// Reads a save, reattaching the executable's tables. The world comes back
/// with `loadedgame` set, as `LoadTheGame` leaves it.
pub fn read(
    bytes: &[u8],
    data: Arc<GameData>,
    trig: Arc<Tables>,
) -> Result<(SaveHeader, World), SaveError> {
    let (header, extra, body) = split(bytes)?;
    let mut w: World = postcard::from_bytes(body)?;
    w.level = match extra {
        Some(e) => e.level,
        None => LevelProfile::stock(w.gamestate.mapon, data.cd),
    };
    w.data = data;
    w.trig = trig;
    w.loadedgame = true;
    Ok((header, w))
}
