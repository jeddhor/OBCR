//! Static objects: `SpawnStatic`, `PlaceItemType` (`WOLFSRC/WL_ACT1.C`;
//! OBC `SpawnStatic` ghidra 2281:0011, `PlaceItemType` 2281:020f).
//! DELTA-UNCHECKED: item kinds per docs/re/LEVEL_CODES.md.

use crate::world::{Occ, Static, World, fl};

/// `MAXSTATS`. DELTA-UNCHECKED (Wolf3D value).
pub const MAXSTATS: usize = 400;

/// `statinfo` kinds: 0 dressing, 1 blocking, others are pickup item numbers.
pub const KIND_DRESSING: i16 = 0;
pub const KIND_BLOCK: i16 = 1;

impl World {
    /// `SpawnStatic(x, y, type)`: static `type` from `statinfo`.
    pub fn spawn_static(&mut self, x: u8, y: u8, kind_index: usize) {
        let Some(info) = self.data.statinfo.get(kind_index).copied() else {
            return;
        };
        if self.statics.len() >= MAXSTATS {
            return;
        }
        let mut s = Static { tilex: x, tiley: y, shape: info.shape, flags: 0, item: info.kind };
        match info.kind {
            KIND_BLOCK => self.set_actorat(i32::from(x), i32::from(y), Occ::Mark(1)),
            KIND_DRESSING => {}
            _ => s.flags = fl::BONUS,
        }
        self.statics.push(s);
    }

    /// `PlaceItemType(item, x, y)`: drops a bonus item, reusing a removed slot.
    pub fn place_item_type(&mut self, item: i16, x: u8, y: u8) {
        let Some(info) = self.data.statinfo.iter().find(|s| s.kind == item).copied() else {
            return;
        };
        let s = Static { tilex: x, tiley: y, shape: info.shape, flags: fl::BONUS, item };
        if let Some(slot) = self.statics.iter_mut().find(|s| s.shape == -1) {
            *slot = s;
        } else if self.statics.len() < MAXSTATS {
            self.statics.push(s);
        }
    }
}
