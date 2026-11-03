//! Allies: spawning, follow and disperse modes, switching (docs/re/ALLIES.md).
//!
//! STUB: interface fixed by the simulation skeleton; implementation pending.

use crate::data::Func;
/// Module state kept in `World::ally`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AlliesState {}

use crate::world::{ObjId, TicCmd, World};

/// Runs an ally state function.
pub fn call(w: &mut World, id: ObjId, f: Func) {
    let _ = (w, id, f);
}

/// `SpawnAlly(x, y, dir)` (ghidra 2634:00e4).
pub fn spawn_ally(w: &mut World, x: u16, y: u16, dir: u16) {
    let _ = (w, x, y, dir);
}

/// Ally keys from CheckKeys: F (mode), A (switch), S (switch and stay).
pub fn handle_keys(w: &mut World, cmd: &TicCmd) {
    let _ = (w, cmd);
}

/// The ally an enemy shoots instead of the player (`FUN_2634_036e`), if any.
pub fn enemy_target(w: &mut World, enemy: ObjId) -> Option<ObjId> {
    let _ = (w, enemy);
    None
}

/// Line of sight between an ally and an enemy (`FUN_2634_0412`).
pub fn can_see(w: &mut World, ally: ObjId, enemy: ObjId) -> bool {
    let _ = (w, ally, enemy);
    false
}
