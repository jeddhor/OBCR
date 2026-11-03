//! The player: controls, movement, collision, use, weapons, pickups, damage
//! (`WOLFSRC/WL_AGENT.C`, `WL_PLAY.C`; OBC deltas in docs/re/PLAYER.md).
//!
//! STUB: interface fixed by the simulation skeleton; implementation pending.

use crate::world::{ObjId, TicCmd, World};

/// `SpawnPlayer(x, y, dir)` (ghidra 208c:1324); `dir` 0..3 = north, east, south, west (codes 19–22).
pub fn spawn_player(w: &mut World, x: u16, y: u16, dir: u16) {
    let _ = (w, x, y, dir);
}

/// `PollControls` replacement: loads `controlx`, `controly`, `buttonstate` from a tic command.
pub fn apply_controls(w: &mut World, cmd: &TicCmd) {
    let _ = (w, cmd);
}

/// `T_Player` (ghidra 208c:1bfb).
pub fn t_player(w: &mut World, id: ObjId) {
    let _ = (w, id);
}

/// `T_Attack` (ghidra 208c:18dc).
pub fn t_attack(w: &mut World, id: ObjId) {
    let _ = (w, id);
}

/// `TakeDamage(points, attacker)` (ghidra 208c:0494).
pub fn take_damage(w: &mut World, points: i16, attacker: Option<ObjId>) {
    let _ = (w, points, attacker);
}

/// `GivePoints` (ghidra 208c:0614).
pub fn give_points(w: &mut World, points: i32) {
    let _ = (w, points);
}

/// `HealSelf` (ghidra 208c:05f4).
pub fn heal_self(w: &mut World, points: i16) {
    let _ = (w, points);
}

/// `GetBonus` (ghidra 208c:074f) for static `index`.
pub fn get_bonus(w: &mut World, index: usize) {
    let _ = (w, index);
}

/// Per-tic PlayLoop extras before actors run: walk bob, knock-back, health clamp
/// (ghidra 1942:1315, docs/re/PLAYER.md §3.4).
pub fn pre_actors(w: &mut World) {
    let _ = w;
}
