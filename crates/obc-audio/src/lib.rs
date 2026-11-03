//! Sound and music synthesis (DESIGN §7.4). Produces sample buffers; no SDL.
//!
//! The game thread decides *what* plays (priorities and channel rules live in
//! the deterministic game state) and sends [`AudioCommand`]s to a [`Mixer`]
//! running on the audio thread, which synthesizes PC speaker sounds, AdLib
//! effects and IMF music (through an OPL emulator) and digitized samples.

mod mixer;
mod synth;

use std::sync::Arc;

pub use mixer::{Mixer, MixerHandle};
pub use obc_formats::audio::{AdlibSound, ImfCommand, PcSound};

/// Service rates in ticks per second. Wolf3D: PC speaker and AdLib effects at
/// 140 Hz, IMF music at 700 Hz (`WOLFSRC/ID_SD.C`); OBC values per docs/re/AUDIO.md.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rates {
    pub sfx_hz: f64,
    pub imf_hz: f64,
}

impl Default for Rates {
    fn default() -> Self {
        Self { sfx_hz: 140.0, imf_hz: 700.0 }
    }
}

/// Volume levels, each 0.0..=1.0.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Volumes {
    pub master: f32,
    pub sfx: f32,
    pub music: f32,
}

impl Default for Volumes {
    fn default() -> Self {
        Self { master: 0.8, sfx: 1.0, music: 0.7 }
    }
}

/// Stereo position of a digitized sound, as the original's SB Pro panning:
/// 0 = full volume, 15 = silent, per channel (`SD_PositionSound`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Pan {
    pub left: u8,
    pub right: u8,
}

/// Commands from the game to the audio thread.
#[derive(Debug, Clone)]
pub enum AudioCommand {
    /// Starts an AdLib effect on OPL channel 0, replacing any current one.
    PlayAdlib(Arc<AdlibSound>),
    StopAdlib,
    /// Starts a PC speaker effect, replacing any current one.
    PlayPc(Arc<PcSound>),
    StopPc,
    /// Plays a digitized sound (8-bit unsigned mono) on `channel`, replacing what was there.
    PlayDigi {
        channel: usize,
        samples: Arc<[u8]>,
        rate: u32,
        pan: Pan,
    },
    /// Repositions a playing digitized sound.
    PanDigi {
        channel: usize,
        pan: Pan,
    },
    StopDigi {
        channel: usize,
    },
    /// Starts looping IMF music.
    StartMusic(Arc<[ImfCommand]>),
    StopMusic,
    /// `SD_MusicOff` / `SD_MusicOn` for Pause: silences the OPL and holds the
    /// song position; resuming continues from there.
    PauseMusic(bool),
    SetVolumes(Volumes),
    SetRates(Rates),
}

/// Number of digitized channels the mixer provides.
pub const DIGI_CHANNELS: usize = 4;
