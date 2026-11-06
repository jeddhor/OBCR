//! Sound playback rules (`SD_PlaySound` 2b79:15b3, `SetSoundLoc` 1823:0006,
//! `UpdateSoundLoc` 1823:017f; docs/re/AUDIO.md §4).
//!
//! The simulation only asks for sounds; this decides what actually plays.
//! Every sound id has a digitized version (AUDIOMUS n) at 9,009 Hz and an AdLib
//! version whose chunk also supplies the priority. With one digitized voice
//! (the original), a sound that loses the priority test falls back to AdLib.
//! The Enhanced option mixes up to [`DIGI_CHANNELS`] digitized voices.

use std::sync::Arc;

use obc_audio::{AdlibSound, AudioCommand, DIGI_CHANNELS, MixerHandle, Pan, PcSound};
use obc_core::fixed::fixed_by_frac;
use obc_core::trig::Tables;
use obc_sim::data::GameData;

use crate::assets::Assets;

/// Digitized sample rate of `AUDIOMUS.BC` (Q-006).
pub const DIGI_RATE: u32 = 9009;
/// Number of sound ids (`NUMSOUNDS`).
const NUMSOUNDS: usize = 100;
/// First AdLib chunk.
const STARTADLIBSOUNDS: usize = 100;
/// `focallength` passed to `CalcProjection`.
const FOCALLENGTH: i32 = 0x5700;
/// `ATABLEMAX`.
const ATABLEMAX: i32 = 15;

/// Where the listener is (the player), for positioned sounds.
#[derive(Debug, Clone, Copy, Default)]
pub struct Listener {
    pub x: i32,
    pub y: i32,
    pub angle: i16,
}

#[derive(Debug, Clone, Copy, Default)]
struct Voice {
    priority: u16,
    /// Tic at which the sound has finished.
    until: u64,
    /// World position for `UpdateSoundLoc`.
    at: Option<(i32, i32)>,
}

/// Which device plays effects (`SoundMode`/`DigiMode`, set by SETUP in the
/// original): digitized with AdLib fallback, AdLib only, or PC speaker.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Effects {
    #[default]
    Digitized,
    Adlib,
    PcSpeaker,
}

impl Effects {
    pub fn from_setting(s: &str) -> Self {
        match s {
            "adlib" => Self::Adlib,
            "pc" => Self::PcSpeaker,
            _ => Self::Digitized,
        }
    }
}

pub struct SoundManager {
    adlib: Vec<Option<Arc<AdlibSound>>>,
    pc: Vec<Option<Arc<PcSound>>>,
    pub effects: Effects,
    digi: Vec<Option<Arc<[u8]>>>,
    game: Arc<GameData>,
    trig: Arc<Tables>,
    /// Digitized voices in use: 1 (original) up to `DIGI_CHANNELS`.
    channels: usize,
    voices: [Voice; DIGI_CHANNELS],
    adlib_voice: Voice,
    clock: u64,
    pub enabled: bool,
}

impl SoundManager {
    pub fn new(a: &Assets, channels: usize) -> Self {
        let adlib = (0..NUMSOUNDS)
            .map(|i| a.audio.adlib_sound(STARTADLIBSOUNDS + i).ok().map(Arc::new))
            .collect();
        let digi = (0..NUMSOUNDS)
            .map(|i| a.digi.get(i).filter(|d| !d.is_empty()).map(|d| Arc::from(d.as_slice())))
            .collect();
        let pc = (0..NUMSOUNDS).map(|i| a.audio.pc_sound(i).ok().map(Arc::new)).collect();
        Self {
            adlib,
            pc,
            effects: Effects::Digitized,
            digi,
            game: a.game.clone(),
            trig: a.trig.clone(),
            channels: channels.clamp(1, DIGI_CHANNELS),
            voices: [Voice::default(); DIGI_CHANNELS],
            adlib_voice: Voice::default(),
            clock: 0,
            enabled: true,
        }
    }

    pub fn set_channels(&mut self, n: usize) {
        self.channels = n.clamp(1, DIGI_CHANNELS);
    }

    /// Advances the sound clock by `tics` 70 Hz tics.
    pub fn advance(&mut self, tics: u64) {
        self.clock += tics;
    }

    fn playing(&self, v: &Voice) -> bool {
        self.clock < v.until
    }

    /// `SD_PlaySound` / `PlaySoundLocGlobal`.
    pub fn play(&mut self, audio: &MixerHandle, id: u16, at: Option<(i32, i32)>, l: Listener) {
        if !self.enabled {
            return;
        }
        let i = usize::from(id);
        if self.effects == Effects::PcSpeaker {
            // SoundMode 1: the PC chunk supplies sound and priority; no digitized path.
            let Some(Some(pc)) = self.pc.get(i).cloned() else { return };
            if self.playing(&self.adlib_voice) && pc.priority < self.adlib_voice.priority {
                return;
            }
            let ticks = pc.data.len() as u64;
            self.adlib_voice =
                Voice { priority: pc.priority, until: self.clock + ticks.div_ceil(2), at: None };
            audio.send(AudioCommand::PlayPc(pc));
            return;
        }
        let Some(Some(adlib)) = self.adlib.get(i).cloned() else { return };
        let priority = adlib.priority;
        let pan = at.map_or(Pan::default(), |(x, y)| self.sound_loc(l, x, y));

        if self.effects == Effects::Digitized
            && let Some(Some(samples)) = self.digi.get(i).cloned()
        {
            // Free voice first, then the lowest-priority one this sound may interrupt.
            let slot = (0..self.channels).find(|&c| !self.playing(&self.voices[c])).or_else(|| {
                (0..self.channels)
                    .filter(|&c| priority >= self.voices[c].priority)
                    .min_by_key(|&c| (self.voices[c].priority, self.voices[c].until))
            });
            if let Some(c) = slot {
                let len = samples.len() as u64;
                self.voices[c] = Voice {
                    priority,
                    until: self.clock + (len * 70).div_ceil(u64::from(DIGI_RATE)),
                    at,
                };
                audio.send(AudioCommand::PlayDigi { channel: c, samples, rate: DIGI_RATE, pan });
                return;
            }
        }
        // AdLib fallback (the original's behaviour when the digitized test fails).
        if self.playing(&self.adlib_voice) && priority < self.adlib_voice.priority {
            return;
        }
        // AdLib effects run at 140 Hz, one data byte per service tick.
        let ticks = adlib.data.len() as u64;
        self.adlib_voice = Voice { priority, until: self.clock + ticks.div_ceil(2), at: None };
        audio.send(AudioCommand::PlayAdlib(adlib));
    }

    /// `UpdateSoundLoc`: repositions playing positioned sounds.
    pub fn update(&mut self, audio: &MixerHandle, l: Listener) {
        for c in 0..self.channels {
            let v = self.voices[c];
            if let Some((x, y)) = v.at
                && self.playing(&v)
            {
                audio.send(AudioCommand::PanDigi { channel: c, pan: self.sound_loc(l, x, y) });
            }
        }
    }

    /// Stops every effect (`SD_StopSound`).
    pub fn stop_all(&mut self, audio: &MixerHandle) {
        for c in 0..DIGI_CHANNELS {
            audio.send(AudioCommand::StopDigi { channel: c });
        }
        audio.send(AudioCommand::StopAdlib);
        self.voices = [Voice::default(); DIGI_CHANNELS];
        self.adlib_voice = Voice::default();
    }

    /// True while a digitized sound plays (`SD_SoundPlaying` for `SD_WaitSoundDone`).
    pub fn digi_playing(&self) -> bool {
        self.voices[..self.channels].iter().any(|v| self.playing(v))
    }

    /// `SetSoundLoc`: attenuation per channel from the listener's view.
    pub fn sound_loc(&self, l: Listener, gx: i32, gy: i32) -> Pan {
        let angle = usize::from(l.angle.rem_euclid(360) as u16);
        let viewcos = self.trig.cos(angle);
        let viewsin = self.trig.sin(angle);
        let viewx = l.x.wrapping_sub(fixed_by_frac(FOCALLENGTH, viewcos));
        let viewy = l.y.wrapping_add(fixed_by_frac(FOCALLENGTH, viewsin));
        let gx = gx.wrapping_sub(viewx);
        let gy = gy.wrapping_sub(viewy);
        let x = (fixed_by_frac(gx, viewcos).wrapping_sub(fixed_by_frac(gy, viewsin))) >> 16;
        let y = (fixed_by_frac(gy, viewcos).wrapping_add(fixed_by_frac(gx, viewsin))) >> 16;
        let y = y.clamp(-ATABLEMAX, ATABLEMAX - 1);
        let x = x.abs().min(ATABLEMAX - 1);
        let i = (x * 2 * ATABLEMAX + y + ATABLEMAX) as usize;
        Pan {
            left: self.game.pan_left.get(i).copied().unwrap_or(0),
            right: self.game.pan_right.get(i).copied().unwrap_or(0),
        }
    }
}
