//! Animated walls: door-look walls, fans, openings (`SpawnAnimWall`
//! ghidra 2f4d:0847, `OperateAnimWall` 2f4d:08e8, `MoveAnimWalls` 2f4d:0978,
//! `ConnectAnimWallAreas` 2f4d:054a) and OBC push walls (`PushWall` 2281:0ce2,
//! `MovePWalls` 2281:0e58). Spec: docs/re/LEVEL_CODES.md §3.3, §4.4, §4.7.

use crate::level::PUSHABLETILE;
use crate::world::{AREATILE, NUMAREAS, Occ, World};

/// Maximum animated walls (`Quit(703)` above).
pub const MAXANIMWALLS: usize = 32;

/// One animated-wall record (12 bytes at DS:0x46E2 in the original).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct AnimWall {
    pub x: u16,
    pub y: u16,
    /// Frame 0..3 (offset added to the base wall code).
    pub frame: u8,
    /// 0 open, 1 closed, 2 opening, 3 closing, 4 permanently open, 5–7 looping.
    pub state: u8,
    pub kind: u16,
    pub timer: u8,
}

/// Loop timer reload by state 5, 6, 7 (DS:0x3217 + state).
const LOOP_RELOAD: [u8; 3] = [6, 3, 0];

impl World {
    /// `SpawnAnimWall(x, y, type)`.
    pub fn spawn_anim_wall(&mut self, x: u16, y: u16, kind: u16) {
        if self.animwalls.len() >= MAXANIMWALLS {
            return;
        }
        let state = match kind {
            2 => 4,
            3..=5 => (kind + 2) as u8,
            _ => 1,
        };
        self.animwalls.push(AnimWall { x, y, frame: 0, state, kind, timer: 0 });
        if state == 4 {
            self.set_actorat(i32::from(x), i32::from(y), Occ::Empty);
            self.connect_anim_wall_areas(i32::from(x), i32::from(y), true);
        }
    }

    pub fn anim_wall_at(&self, x: i32, y: i32) -> Option<usize> {
        self.animwalls.iter().position(|a| i32::from(a.x) == x && i32::from(a.y) == y)
    }

    /// `OperateAnimWall(x, y)`: Use on a door-look wall.
    pub fn operate_anim_wall(&mut self, x: i32, y: i32) {
        let Some(i) = self.anim_wall_at(x, y) else { return };
        match self.animwalls[i].state {
            1 => self.animwalls[i].state = 2,
            0 if self.actorat(x, y).is_empty() => {
                self.animwalls[i].state = 3;
                self.set_actorat(x, y, Occ::Mark(1));
            }
            _ => {}
        }
    }

    /// `MoveAnimWalls`: once per frame.
    pub fn move_anim_walls(&mut self) {
        for i in 0..self.animwalls.len() {
            let a = self.animwalls[i];
            let (x, y) = (i32::from(a.x), i32::from(a.y));
            let idx = Self::idx(x, y);
            match a.state {
                2 => {
                    if a.frame == 0 && self.tilemap[idx] != 85 {
                        self.connect_anim_wall_areas(x, y, true);
                    }
                    self.tilemap[idx] = self.tilemap[idx].wrapping_add(1);
                    let w = &mut self.animwalls[i];
                    w.frame += 1;
                    if w.frame >= 3 {
                        w.state = 0;
                        self.actorat[idx] = Occ::Empty;
                    }
                }
                3 => {
                    self.tilemap[idx] = self.tilemap[idx].wrapping_sub(1);
                    let w = &mut self.animwalls[i];
                    w.frame = w.frame.saturating_sub(1);
                    if w.frame == 0 {
                        w.state = 1;
                        if self.tilemap[idx] != 85 {
                            self.connect_anim_wall_areas(x, y, false);
                        }
                    }
                }
                5..=7 => {
                    let w = &mut self.animwalls[i];
                    if w.timer == 0 {
                        w.timer = LOOP_RELOAD[(w.state - 5) as usize];
                        if w.frame >= 3 {
                            w.frame = 0;
                            self.tilemap[idx] = self.tilemap[idx].wrapping_sub(3);
                        } else {
                            w.frame += 1;
                            self.tilemap[idx] = self.tilemap[idx].wrapping_add(1);
                        }
                    } else {
                        w.timer -= 1;
                    }
                }
                _ => {}
            }
        }
    }

    /// `ConnectAnimWallAreas`: link the areas on both sides (east/west if both
    /// are open, otherwise north/south).
    pub fn connect_anim_wall_areas(&mut self, x: i32, y: i32, open: bool) {
        let (a, b) = if self.tile(x - 1, y) == 0 && self.tile(x + 1, y) == 0 {
            (self.mapspot(x - 1, y, 0), self.mapspot(x + 1, y, 0))
        } else {
            (self.mapspot(x, y - 1, 0), self.mapspot(x, y + 1, 0))
        };
        let (a, b) = (a.wrapping_sub(AREATILE) as usize, b.wrapping_sub(AREATILE) as usize);
        if a >= NUMAREAS || b >= NUMAREAS {
            return;
        }
        let d = if open { 1u8 } else { u8::MAX };
        self.areaconnect[a][b] = self.areaconnect[a][b].wrapping_add(d);
        self.areaconnect[b][a] = self.areaconnect[b][a].wrapping_add(d);
        self.connect_areas();
    }

    /// `PushWall(x, y, dir)`; `dir` 0 N, 1 E, 2 S, 3 W.
    pub fn push_wall(&mut self, x: i32, y: i32, dir: u16) {
        if self.pwall.state != 0 {
            return;
        }
        let oldtile = self.tile(x, y);
        if oldtile == 0 {
            return;
        }
        let (nx, ny) = step(x, y, dir);
        if !self.actorat(nx, ny).is_empty() {
            return;
        }
        self.set_actorat(nx, ny, Occ::Mark(u16::from(oldtile)));
        if World::in_map(nx, ny) {
            self.tilemap[Self::idx(nx, ny)] = oldtile;
        }
        if self.mapspot(x, y, 1) == PUSHABLETILE {
            self.gamestate.secretcount += 1;
        }
        self.pwall = crate::world::PushWall {
            state: 1,
            pos: 0,
            x: x as u16,
            y: y as u16,
            dir,
            tile: oldtile,
        };
        self.tilemap[Self::idx(x, y)] = oldtile | 0x80;
        self.play_sound(0x53);
    }

    /// `MovePWalls`: two cells, then the wall rests.
    pub fn move_pwalls(&mut self) {
        if self.pwall.state == 0 {
            return;
        }
        let oldblock = self.pwall.state / 128;
        self.pwall.state += self.tics as u16;
        if self.pwall.state / 128 != oldblock {
            let (x, y) = (i32::from(self.pwall.x), i32::from(self.pwall.y));
            let oldtile = self.pwall.tile;
            self.tilemap[Self::idx(x, y)] = 0;
            self.actorat[Self::idx(x, y)] = Occ::Empty;
            let area = u16::from(self.plr().areanumber) + AREATILE;
            self.set_mapspot(x, y, 0, area);
            self.set_mapspot(x, y, 1, 0);
            let (nx, ny) = step(x, y, self.pwall.dir);
            self.set_mapspot(nx, ny, 1, PUSHABLETILE);
            if self.pwall.state > 256 {
                self.tilemap[Self::idx(nx, ny)] = oldtile;
                self.pwall.state = 0;
                return;
            }
            self.pwall.x = nx as u16;
            self.pwall.y = ny as u16;
            let (fx, fy) = step(nx, ny, self.pwall.dir);
            if !self.actorat(fx, fy).is_empty() {
                self.tilemap[Self::idx(nx, ny)] = oldtile;
                self.pwall.state = 0;
                return;
            }
            self.set_actorat(fx, fy, Occ::Mark(u16::from(oldtile)));
            if World::in_map(fx, fy) {
                self.tilemap[Self::idx(fx, fy)] = oldtile;
            }
            self.tilemap[Self::idx(nx, ny)] = oldtile | 0x80;
        }
        self.pwall.pos = (self.pwall.state / 2) & 63;
    }
}

fn step(x: i32, y: i32, dir: u16) -> (i32, i32) {
    match dir {
        0 => (x, y - 1),
        1 => (x + 1, y),
        2 => (x, y + 1),
        _ => (x - 1, y),
    }
}
