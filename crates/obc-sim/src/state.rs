//! The actor state machine: `NewState`, `SpawnNewObj`, `DoActor` and the
//! think/action dispatch (`WOLFSRC/WL_STATE.C`, `WL_PLAY.C`; OBC deltas in
//! docs/re/ACTORS.md §4, §6.6).

use crate::actors;
use crate::allies;
use crate::data::{Func, StateId};
use crate::player;
use crate::world::{AREATILE, MAPSIZE, ObjId, Occ, TILEGLOBAL, World, class, dir, fl};

/// The fire "stand" state; `SpawnNewObj` refuses it above 50 objects.
const S_FIRE_STAND: StateId = 0x137E;

impl World {
    /// `NewState` (ghidra 1e43:01ab, no delta).
    pub fn new_state(&mut self, id: ObjId, state: StateId) {
        let tics = self.data.state(state).map_or(0, |s| s.tics);
        let o = self.obj_mut(id);
        o.state = state;
        o.ticcount = tics;
    }

    /// `SpawnNewObj` (ghidra 1e43:0007). Returns the new object, or `None`
    /// when the object list is full (OBC does not quit).
    pub fn spawn_new_obj(&mut self, tilex: u16, tiley: u16, state: StateId) -> Option<ObjId> {
        if state == S_FIRE_STAND && self.objcount > 50 {
            return None;
        }
        let (mut tx, mut ty) = (tilex, tiley);
        if self.gamestate.difficulty == 3 {
            // Random game type: scatter up to 24 tiles (RandomNear, ghidra 2f4d:1388).
            let mut found = false;
            for _ in 0..10 {
                let x = self.random_near(tilex as i16, 24);
                let y = self.random_near(tiley as i16, 24);
                if self.actorat(x as i32, y as i32).is_empty() {
                    // The original scans every slot, removed statics included.
                    if let Some(si) =
                        self.statics.iter().position(|s| s.tilex == x as u8 && s.tiley == y as u8)
                    {
                        // A static occupies the spot; a fire ignites bonus items.
                        if state == S_FIRE_STAND && self.statics[si].flags & fl::BONUS != 0 {
                            actors::ignite(self, tilex as i32, tiley as i32);
                        }
                        return None;
                    }
                    tx = x as u16;
                    ty = y as u16;
                    found = true;
                    break;
                }
            }
            if !found {
                return None;
            }
        }
        let id = self.get_new_actor()?;
        let tics = self.data.state(state).map_or(0, |s| s.tics);
        // US_RndT is only consumed when the state has a duration.
        let ticcount = if tics != 0 { self.rnd() % tics } else { 0 };
        let area = self.mapspot(tx as i32, ty as i32, 0).wrapping_sub(AREATILE) as u8;
        let o = self.obj_mut(id);
        o.state = state;
        o.ticcount = ticcount;
        o.tilex = tx;
        o.tiley = ty;
        o.x = (i32::from(tx) << 16) + TILEGLOBAL / 2;
        o.y = (i32::from(ty) << 16) + TILEGLOBAL / 2;
        o.dir = dir::NODIR;
        o.flags = 0;
        o.areanumber = area;
        self.set_actorat(tx as i32, ty as i32, Occ::Obj(id));
        Some(id)
    }

    /// `RandomNear` (ghidra 2f4d:1388): `|v ± US_RndT() % range| % 64`.
    pub fn random_near(&mut self, v: i16, range: i16) -> i16 {
        let sign = if self.rnd() > 128 { -1 } else { 1 };
        let r = self.rnd() % range;
        (v + sign * r).abs() % MAPSIZE as i16
    }

    /// Index of the live static at a tile, if any.
    pub fn static_at(&self, x: u8, y: u8) -> Option<usize> {
        self.statics.iter().position(|s| s.shape != -1 && s.tilex == x && s.tiley == y)
    }

    /// Runs a state function for `id`.
    pub fn call(&mut self, id: ObjId, f: Func) {
        match f {
            Func::TPlayer => player::t_player(self, id),
            Func::TAttack => player::t_attack(self, id),
            Func::TAlly | Func::AAlly => allies::call(self, id, f),
            Func::ARemove => self.obj_mut(id).state = 0,
            Func::ANop => {}
            Func::Unknown(addr) => self
                .emit(crate::events::SimEvent::Fault(format!("unknown state function {addr:08x}"))),
            _ => actors::call(self, id, f),
        }
    }

    /// `DoActor` (ghidra 1942:10f5).
    pub fn do_actor(&mut self, id: ObjId, index: usize) {
        let o = *self.obj(id);
        // BEWITCHED freezes every enemy class (ghidra 1942:12e1).
        if self.bewitched && matches!(o.obclass, 2..=4 | 6..=17 | 22 | 23) {
            return;
        }
        if let Some(p) = self.actorpos.get_mut(index) {
            *p = 0;
        }
        if o.active == 0 && !self.area_by_player(o.areanumber) {
            return;
        }
        if o.flags & (fl::NONMARK | fl::NEVERMARK) == 0 {
            self.set_actorat(o.tilex as i32, o.tiley as i32, Occ::Empty);
        }
        // 358f:19fd: a remote player's puppet follows its packets.
        let o = if self.gamestate.difficulty >= 4 && index != 0 && matches!(o.obclass, 4 | 5 | 12) {
            crate::net::sync_puppet(self, id);
            *self.obj(id)
        } else {
            o
        };
        if o.obclass != class::ALLY
            && id != self.player
            && let Some(p) = self.actorpos.get_mut(index)
        {
            *p = (o.tilex << 8) | (o.tiley & 0xFF);
        }

        if o.ticcount == 0 {
            if !self.run_think(id) {
                return;
            }
            self.mark(id);
            return;
        }

        self.obj_mut(id).ticcount -= self.tics;
        while self.obj(id).ticcount <= 0 {
            let state = self.obj(id).state;
            let Some(s) = self.data.state(state).copied() else {
                self.remove_obj(id);
                return;
            };
            if let Some(action) = s.action {
                self.call(id, action);
                if self.obj(id).state == 0 {
                    self.remove_obj(id);
                    return;
                }
            }
            // The action may have changed the state; follow the current one.
            let cur = self.obj(id).state;
            let next = self.data.state(cur).map_or(0, |s| s.next);
            self.obj_mut(id).state = next;
            if next == 0 {
                self.remove_obj(id);
                return;
            }
            let tictime = self.data.state(next).map_or(0, |s| s.tics);
            if tictime == 0 {
                self.obj_mut(id).ticcount = 0;
                break;
            }
            self.obj_mut(id).ticcount += tictime;
        }
        if !self.run_think(id) {
            return;
        }
        self.mark(id);
    }

    /// Runs the current state's think. Returns false if the object was removed.
    fn run_think(&mut self, id: ObjId) -> bool {
        let think = self.data.state(self.obj(id).state).and_then(|s| s.think);
        if let Some(t) = think {
            self.call(id, t);
            if self.obj(id).state == 0 {
                self.remove_obj(id);
                return false;
            }
        }
        true
    }

    /// The end of `DoActor`: mark the object in `actorat`.
    fn mark(&mut self, id: ObjId) {
        let o = *self.obj(id);
        if o.flags & fl::NEVERMARK != 0 {
            return;
        }
        if o.flags & fl::NONMARK != 0 && !self.actorat(o.tilex as i32, o.tiley as i32).is_empty() {
            return;
        }
        self.set_actorat(o.tilex as i32, o.tiley as i32, Occ::Obj(id));
    }

    pub fn area_by_player(&self, area: u8) -> bool {
        self.areabyplayer.get(area as usize).copied().unwrap_or(false)
    }
}
