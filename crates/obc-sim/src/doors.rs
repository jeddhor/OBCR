//! Doors and area connectivity (`WOLFSRC/WL_ACT1.C`; OBC `SpawnDoor`
//! ghidra 2281:0437, `OperateDoor` 2281:07f3).
//! DELTA-UNCHECKED: door kinds, locks and sounds per docs/re/LEVEL_CODES.md.

use crate::world::{AREATILE, DoorAction, NUMAREAS, Occ, World};

/// Tics a door stays open (`OPENTICS`).
pub const OPENTICS: i16 = 300;
/// `MAXDOORS`.
pub const MAXDOORS: usize = 64;

impl World {
    /// `ConnectAreas`: flood `areabyplayer` from the player's area through open doors.
    pub fn connect_areas(&mut self) {
        self.areabyplayer = [false; NUMAREAS];
        let a = self.plr().areanumber as usize;
        if a < NUMAREAS {
            self.areabyplayer[a] = true;
            self.recursive_connect(a);
        }
    }

    fn recursive_connect(&mut self, area: usize) {
        for i in 0..NUMAREAS {
            if self.areaconnect[area][i] != 0 && !self.areabyplayer[i] {
                self.areabyplayer[i] = true;
                self.recursive_connect(i);
            }
        }
    }

    /// `OpenDoor`.
    pub fn open_door(&mut self, door: usize) {
        let Some(d) = self.doors.get_mut(door) else { return };
        if d.action == DoorAction::Open {
            d.ticcount = 0;
        } else {
            d.action = DoorAction::Opening;
        }
    }

    /// `CloseDoor`: refuses while anything stands in or beside the doorway.
    pub fn close_door(&mut self, door: usize) {
        let Some(d) = self.doors.get(door).copied() else { return };
        let (tx, ty) = (i32::from(d.tilex), i32::from(d.tiley));
        if !self.actorat(tx, ty).is_empty() {
            return;
        }
        let p = *self.plr();
        if i32::from(p.tilex) == tx && i32::from(p.tiley) == ty {
            return;
        }
        const MINDIST: i32 = 0x5800;
        if d.vertical {
            if i32::from(p.tiley) == ty
                && ((p.x + MINDIST) >> 16 == tx || (p.x - MINDIST) >> 16 == tx)
            {
                return;
            }
            if let Occ::Obj(o) = self.actorat(tx - 1, ty)
                && (self.obj(o).x + MINDIST) >> 16 == tx
            {
                return;
            }
            if let Occ::Obj(o) = self.actorat(tx + 1, ty)
                && (self.obj(o).x - MINDIST) >> 16 == tx
            {
                return;
            }
        } else {
            if i32::from(p.tilex) == tx
                && ((p.y + MINDIST) >> 16 == ty || (p.y - MINDIST) >> 16 == ty)
            {
                return;
            }
            if let Occ::Obj(o) = self.actorat(tx, ty - 1)
                && (self.obj(o).y + MINDIST) >> 16 == ty
            {
                return;
            }
            if let Occ::Obj(o) = self.actorat(tx, ty + 1)
                && (self.obj(o).y - MINDIST) >> 16 == ty
            {
                return;
            }
        }
        let area = self.mapspot(tx, ty, 0).wrapping_sub(AREATILE) as u8;
        if self.area_by_player(area) {
            self.play_sound_tile(sounds::DOOR_CLOSE, tx, ty);
        }
        self.doors[door].action = DoorAction::Closing;
        self.set_actorat(tx, ty, Occ::Mark(0x80 | door as u16));
    }

    /// `OperateDoor`.
    pub fn operate_door(&mut self, door: usize) {
        let Some(d) = self.doors.get(door).copied() else { return };
        match d.action {
            DoorAction::Closed | DoorAction::Closing => self.open_door(door),
            DoorAction::Open | DoorAction::Opening => self.close_door(door),
        }
    }

    fn door_areas(&self, door: usize) -> (usize, usize) {
        let d = self.doors[door];
        let (x, y) = (i32::from(d.tilex), i32::from(d.tiley));
        let (a1, a2) = if d.vertical {
            (self.mapspot(x + 1, y, 0), self.mapspot(x - 1, y, 0))
        } else {
            (self.mapspot(x, y - 1, 0), self.mapspot(x, y + 1, 0))
        };
        (a1.wrapping_sub(AREATILE) as usize, a2.wrapping_sub(AREATILE) as usize)
    }

    fn door_opening(&mut self, door: usize) {
        let mut position = i32::from(self.doors[door].position);
        if position == 0 {
            let (a1, a2) = self.door_areas(door);
            if a1 < NUMAREAS && a2 < NUMAREAS {
                self.areaconnect[a1][a2] = self.areaconnect[a1][a2].wrapping_add(1);
                self.areaconnect[a2][a1] = self.areaconnect[a2][a1].wrapping_add(1);
                self.connect_areas();
                if self.areabyplayer[a1] {
                    let d = self.doors[door];
                    self.play_sound_tile(sounds::DOOR_OPEN, i32::from(d.tilex), i32::from(d.tiley));
                }
            }
        }
        position += i32::from(self.tics) << 10;
        if position >= 0xFFFF {
            position = 0xFFFF;
            let d = &mut self.doors[door];
            d.ticcount = 0;
            d.action = DoorAction::Open;
            let (x, y) = (i32::from(d.tilex), i32::from(d.tiley));
            self.set_actorat(x, y, Occ::Empty);
        }
        self.doors[door].position = position as u16;
    }

    fn door_closing(&mut self, door: usize) {
        let d = self.doors[door];
        let (x, y) = (i32::from(d.tilex), i32::from(d.tiley));
        let p = *self.plr();
        if self.actorat(x, y) != Occ::Mark(0x80 | door as u16)
            || (i32::from(p.tilex) == x && i32::from(p.tiley) == y)
        {
            self.open_door(door);
            return;
        }
        let mut position = i32::from(d.position) - (i32::from(self.tics) << 10);
        if position <= 0 {
            position = 0;
            self.doors[door].action = DoorAction::Closed;
            let (a1, a2) = self.door_areas(door);
            if a1 < NUMAREAS && a2 < NUMAREAS {
                self.areaconnect[a1][a2] = self.areaconnect[a1][a2].wrapping_sub(1);
                self.areaconnect[a2][a1] = self.areaconnect[a2][a1].wrapping_sub(1);
                self.connect_areas();
            }
        }
        self.doors[door].position = position as u16;
    }

    /// `MoveDoors`.
    pub fn move_doors(&mut self) {
        for door in 0..self.doors.len() {
            match self.doors[door].action {
                DoorAction::Open => {
                    self.doors[door].ticcount += self.tics;
                    if self.doors[door].ticcount >= OPENTICS {
                        self.close_door(door);
                    }
                }
                DoorAction::Opening => self.door_opening(door),
                DoorAction::Closing => self.door_closing(door),
                DoorAction::Closed => {}
            }
        }
    }
}

/// Door sound ids. DELTA-UNCHECKED: OBC ids per docs/re/LEVEL_CODES.md / AUDIO.md.
pub mod sounds {
    pub const DOOR_OPEN: u16 = 62;
    pub const DOOR_CLOSE: u16 = 63;
}
