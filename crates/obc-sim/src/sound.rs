//! Sound requests (`SD_PlaySound`, `PlaySoundAt` ghidra 1823:0106). Sound id
//! `n` plays AUDIOMUS digitized sound `n` (docs/re/AUDIO.md).

use crate::events::SimEvent;
use crate::world::World;

impl World {
    /// `SD_PlaySound(id)`: non-positional.
    pub fn play_sound(&mut self, id: u16) {
        self.emit(SimEvent::Sound(id));
    }

    /// `PlaySoundAt(id, x, y)` (Wolf3D `PlaySoundLocGlobal`).
    pub fn play_sound_at(&mut self, id: u16, x: i32, y: i32) {
        self.emit(SimEvent::SoundAt { id, x, y });
    }

    /// Sound at a tile center (`PlaySoundLocTile`).
    pub fn play_sound_tile(&mut self, id: u16, tx: i32, ty: i32) {
        self.play_sound_at(id, (tx << 16) + 0x8000, (ty << 16) + 0x8000);
    }
}
