//! OBCR save files (DESIGN §7.7, M11): a magic, a postcard header and a
//! postcard-serialized [`World`]. Not the original `.SAV` layout.
//!
//! ```text
//! "OBCRSAVE"  u32 LE header length  header (postcard)  world (postcard)
//! ```

use std::sync::Arc;

use obc_core::trig::Tables;
use serde::{Deserialize, Serialize};

use crate::data::GameData;
use crate::world::World;

pub const MAGIC: &[u8; 8] = b"OBCRSAVE";
/// Bumped whenever `World` or the header changes shape.
pub const FORMAT: u32 = 1;

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
    #[error("save format {0} is not supported (expected {FORMAT})")]
    Format(u32),
    #[error("corrupt save: {0}")]
    Corrupt(#[from] postcard::Error),
}

/// Serializes `w` with `header`.
pub fn write(w: &World, header: &SaveHeader) -> Result<Vec<u8>, SaveError> {
    let head = postcard::to_stdvec(header)?;
    let mut out = Vec::with_capacity(64 * 1024);
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&(head.len() as u32).to_le_bytes());
    out.extend_from_slice(&head);
    out.extend_from_slice(&postcard::to_stdvec(w)?);
    Ok(out)
}

fn split(bytes: &[u8]) -> Result<(SaveHeader, &[u8]), SaveError> {
    if bytes.len() < 12 || &bytes[..8] != MAGIC {
        return Err(SaveError::BadMagic);
    }
    let len = u32::from_le_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]) as usize;
    let head = bytes.get(12..12 + len).ok_or(SaveError::BadMagic)?;
    let header: SaveHeader = postcard::from_bytes(head)?;
    if header.format != FORMAT {
        return Err(SaveError::Format(header.format));
    }
    Ok((header, &bytes[12 + len..]))
}

/// Reads only the header (for slot lists).
pub fn read_header(bytes: &[u8]) -> Result<SaveHeader, SaveError> {
    split(bytes).map(|(h, _)| h)
}

/// Reads a save, reattaching the executable's tables. The world comes back
/// with `loadedgame` set, as `LoadTheGame` leaves it.
pub fn read(
    bytes: &[u8],
    data: Arc<GameData>,
    trig: Arc<Tables>,
) -> Result<(SaveHeader, World), SaveError> {
    let (header, body) = split(bytes)?;
    let mut w: World = postcard::from_bytes(body)?;
    w.data = data;
    w.trig = trig;
    w.loadedgame = true;
    Ok((header, w))
}
