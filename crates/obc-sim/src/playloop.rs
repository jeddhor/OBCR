//! One frame of `PlayLoop` (ghidra 1942:1315) in the original order.
//!
//! A frame advances `tics` tics (ADR-013: Classic runs 2-tic frames at 35 Hz;
//! replays may feed the original's recorded `tics`).

use crate::actors;
use crate::allies;
use crate::events::SimEvent;
use crate::player;
use crate::world::{TicCmd, World, ex};

impl World {
    /// Runs one frame with `tics` tics.
    pub fn frame(&mut self, cmd: &TicCmd, tics: i16) {
        self.tics = tics.max(1);
        self.tic = self.tic.wrapping_add(tics as u32);

        // PollControls, then the walk/knock-back extras.
        player::apply_controls(self, cmd);
        player::pre_actors(self);

        self.move_doors();
        self.move_pwalls();
        self.move_anim_walls();

        // Actors in list order, starting with the player.
        self.actorpos = [0; crate::world::MAXACTORS];
        let mut index = 0;
        let mut cur = Some(self.player);
        while let Some(id) = cur {
            self.actorindex = index;
            self.do_actor(id, index);
            // As the original, read `ob->next` after the call: RemoveObj leaves it
            // intact, and objects spawned at the list end run this frame.
            cur = self.obj(id).next;
            index += 1;
            if index > crate::world::MAXACTORS {
                break;
            }
        }

        // ThreeDRefresh's gameplay side effects.
        self.visibility_pass();
        self.gamestate.timecount = self.gamestate.timecount.wrapping_add(i32::from(self.tics));

        // Quake: toggles the darkness flag each frame (ghidra 1942:1660).
        if self.quake > 0 {
            self.quake -= 1;
            if self.shake == 0 {
                self.shake = 4;
            }
            self.dark = !self.dark;
            if self.quake == 0 {
                if self.rnd() % 30 == 0 {
                    self.emit(SimEvent::Lighting(2));
                }
                self.dark = false;
            }
        }

        // Wall-break animation and the health cap.
        player::post_actors(self);

        // Random event (1942:18c1): while the face has not appeared (or in
        // bogus mode), a 1-in-256 × 1-in-15 roll with fire released.
        if (self.gamestate.cheatmode || !self.act.face_spawned)
            && self.rnd() == 0
            && self.rnd() % 15 == 0
            && !self.buttonheld[0]
        {
            actors::spawn_face_apparition(self);
            self.act.face_spawned = true;
        }
        player::check_keys(self);
    }

    /// `PlayLoop` entry (ghidra 1942:1315, GAMEFLOW §5.1): the player resets
    /// ([`player::play_loop_start`]), sewers start dark, allies are counted.
    /// Returns the lighting mode: 2 when
    /// the level's generator switch is off (a tile 96 in `tilemap`), else 1.
    pub fn start_play_loop(&mut self) -> u8 {
        player::play_loop_start(self);
        self.dark = (0..=4).contains(&self.gamestate.mapon);
        allies::init_allies(self);
        self.lighting = if self.tilemap.contains(&96) { 2 } else { 1 };
        self.lighting
    }

    /// Start of `Died` (ghidra 1823:0cda): hands away, random death scream
    /// (table DS:0x0918 = 22–26, one `US_RndT`). Returns the angle to turn to,
    /// facing the killer, if one is recorded.
    pub fn start_death(&mut self) -> Option<i16> {
        self.gamestate.weapon = -1;
        let id = 22 + (self.rnd() % 5) as u16;
        self.play_sound(id);
        let k = self.killerobj?;
        let (p, o) = (self.plr(), self.obj(k));
        let dx = f64::from(o.x - p.x);
        let dy = f64::from(p.y - o.y);
        let mut fangle = dy.atan2(dx);
        if fangle < 0.0 {
            fangle += std::f64::consts::TAU;
        }
        Some((fangle / std::f64::consts::TAU * 360.0) as i16 % 360)
    }

    /// One step of the death swing toward `target` (`DEATHROTATE` 4°/tic, the
    /// shorter way round). Returns true when facing it.
    pub fn death_turn(&mut self, target: i16, tics: i16) -> bool {
        let a = self.plr().angle;
        if a == target {
            return true;
        }
        let clockwise = (target - a).rem_euclid(360);
        let counter = (a - target).rem_euclid(360);
        let step = 4 * tics;
        let p = self.player;
        let angle =
            if clockwise < counter { a + step.min(clockwise) } else { a - step.min(counter) };
        self.obj_mut(p).angle = angle.rem_euclid(360);
        self.obj(p).angle == target
    }

    /// Whether the level loop should end.
    pub fn level_done(&self) -> bool {
        self.playstate != ex::STILLPLAYING
    }
}
