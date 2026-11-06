//! `CONFIG.DAT`: high scores (CONFIRMED) and settings (layout open, Q-014;
//! docs/re/DATA_FORMATS.md §7).

use crate::bytes::{fixed_str, i32_at, slice, u16_at};
use crate::error::Result;

/// Number of high-score entries (`MaxScores`).
pub const NUM_SCORES: usize = 7;
/// Bytes per high-score record: `name[20]`, `i32 score`, `u16 completed`, `u16 episode`.
pub const SCORE_BYTES: usize = 28;
/// Name field length including the terminating NUL.
pub const NAME_BYTES: usize = 20;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HighScore {
    pub name: String,
    pub score: i32,
    pub completed: u16,
    pub episode: u16,
}

/// Parses the seven high-score records at the start of `CONFIG.DAT`
/// (also the layout of the default table inside `COUNT.EXE`).
pub fn parse_high_scores(data: &[u8]) -> Result<Vec<HighScore>> {
    (0..NUM_SCORES)
        .map(|i| {
            let r = slice(data, i * SCORE_BYTES, SCORE_BYTES, "high score")?;
            Ok(HighScore {
                name: fixed_str(&r[..NAME_BYTES]),
                score: i32_at(r, 20, "high score")?,
                completed: u16_at(r, 24, "high score")?,
                episode: u16_at(r, 26, "high score")?,
            })
        })
        .collect()
}
