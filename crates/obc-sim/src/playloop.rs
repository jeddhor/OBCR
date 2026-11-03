//! One frame of `PlayLoop` (ghidra 1942:1315) in the original order.
//!
//! A frame advances `tics` tics (ADR-013: Classic runs 2-tic frames at 35 Hz;
//! replays may feed the original's recorded `tics`).

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
        self.madenoise = false;

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

        if self.gamestate.health > 999 {
            self.gamestate.health = 999;
        }

        // CheckKeys: ally orders and tremors toggle.
        allies::handle_keys(self, cmd);
        if cmd.toggle_tremors {
            self.tremors = !self.tremors;
            let msg = if self.tremors { "TREMORS ON" } else { "TREMORS OFF" };
            self.emit(SimEvent::Message(msg.into()));
        }
    }

    /// Whether the level loop should end.
    pub fn level_done(&self) -> bool {
        self.playstate != ex::STILLPLAYING
    }
}
