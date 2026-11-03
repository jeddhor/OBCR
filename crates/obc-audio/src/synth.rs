//! Tick-driven sequencers ported from `WOLFSRC/ID_SD.C`.

use std::sync::Arc;

use nuked_opl3::Opl3Chip;
use obc_formats::audio::{AdlibSound, ImfCommand, PcSound};

/// OPL register bases (`alChar` etc. in `WOLFSRC/ID_SD.H`).
const AL_CHAR: u16 = 0x20;
const AL_SCALE: u16 = 0x40;
const AL_ATTACK: u16 = 0x60;
const AL_SUS: u16 = 0x80;
const AL_FREQ_L: u16 = 0xA0;
const AL_FREQ_H: u16 = 0xB0;
const AL_FEED_CON: u16 = 0xC0;
const AL_WAVE: u16 = 0xE0;
/// Operator offsets of channel 0 (`modifiers[0]`, `carriers[0]`).
const MODIFIER: u16 = 0;
const CARRIER: u16 = 3;

/// Initializes the OPL the way `SDL_StartAL` does.
pub fn init_opl(opl: &mut Opl3Chip) {
    opl.write_register(0x01, 0x20); // enable waveform select
    opl.write_register(0x08, 0x00);
    opl.write_register(0xBD, 0x00); // alEffects
}

/// An AdLib effect in progress (`SDL_ALPlaySound` / `SDL_ALService`).
pub struct AdlibSfx {
    sound: Arc<AdlibSound>,
    pos: usize,
    block: u8,
}

impl AdlibSfx {
    pub fn start(opl: &mut Opl3Chip, sound: Arc<AdlibSound>) -> Self {
        opl.write_register(AL_FREQ_H, 0);
        let i = &sound.instrument;
        opl.write_register(MODIFIER + AL_CHAR, i.m_char);
        opl.write_register(MODIFIER + AL_SCALE, i.m_scale);
        opl.write_register(MODIFIER + AL_ATTACK, i.m_attack);
        opl.write_register(MODIFIER + AL_SUS, i.m_sus);
        opl.write_register(MODIFIER + AL_WAVE, i.m_wave);
        opl.write_register(CARRIER + AL_CHAR, i.c_char);
        opl.write_register(CARRIER + AL_SCALE, i.c_scale);
        opl.write_register(CARRIER + AL_ATTACK, i.c_attack);
        opl.write_register(CARRIER + AL_SUS, i.c_sus);
        opl.write_register(CARRIER + AL_WAVE, i.c_wave);
        // The original writes 0 here for old MUSE compatibility, not `nConn`.
        opl.write_register(AL_FEED_CON, 0);
        let block = ((sound.block & 7) << 2) | 0x20;
        Self { sound, pos: 0, block }
    }

    /// One service tick. Returns false when finished.
    pub fn tick(&mut self, opl: &mut Opl3Chip) -> bool {
        let Some(&s) = self.sound.data.get(self.pos) else {
            opl.write_register(AL_FREQ_H, 0);
            return false;
        };
        self.pos += 1;
        if s == 0 {
            opl.write_register(AL_FREQ_H, 0);
        } else {
            opl.write_register(AL_FREQ_L, s);
            opl.write_register(AL_FREQ_H, self.block);
        }
        if self.pos >= self.sound.data.len() {
            opl.write_register(AL_FREQ_H, 0);
            return false;
        }
        true
    }

    pub fn stop(opl: &mut Opl3Chip) {
        opl.write_register(AL_FREQ_H, 0);
    }
}

/// A PC speaker effect in progress (`SDL_PCPlaySound` / `SDL_PCService`).
pub struct PcSfx {
    sound: Arc<PcSound>,
    pos: usize,
    /// Current square-wave frequency in Hz (0 = silent).
    pub freq: f64,
}

impl PcSfx {
    pub fn start(sound: Arc<PcSound>) -> Self {
        Self { sound, pos: 0, freq: 0.0 }
    }

    /// One service tick: the next byte selects the PIT divisor (`byte × 60`).
    pub fn tick(&mut self) -> bool {
        const PIT_HZ: f64 = 1_193_181.0;
        let Some(&s) = self.sound.data.get(self.pos) else {
            self.freq = 0.0;
            return false;
        };
        self.pos += 1;
        self.freq = if s == 0 { 0.0 } else { PIT_HZ / (f64::from(s) * 60.0) };
        true
    }
}

/// Looping IMF music (`SDL_ALService` music path).
pub struct ImfPlayer {
    commands: Arc<[ImfCommand]>,
    pos: usize,
    wait: u32,
}

impl ImfPlayer {
    pub fn new(commands: Arc<[ImfCommand]>) -> Self {
        Self { commands, pos: 0, wait: 0 }
    }

    /// One music tick: writes every command whose time has come.
    pub fn tick(&mut self, opl: &mut Opl3Chip) {
        if self.commands.is_empty() {
            return;
        }
        while self.wait == 0 {
            if self.pos >= self.commands.len() {
                self.pos = 0;
            }
            let c = self.commands[self.pos];
            self.pos += 1;
            // Channel 0 belongs to sound effects; music uses channels 1-8.
            opl.write_register(u16::from(c.reg), c.value);
            self.wait = u32::from(c.delay);
            if self.pos >= self.commands.len() && self.wait == 0 {
                self.wait = 1;
            }
        }
        self.wait -= 1;
    }

    /// Silences music (`SD_MusicOff`: effects register off, key off channels 1-8).
    pub fn silence(opl: &mut Opl3Chip) {
        opl.write_register(0xBD, 0);
        for ch in 1..9u16 {
            opl.write_register(AL_FREQ_H + ch, 0);
        }
    }
}
