//! Enemies, projectiles and fire: spawning, AI, damage and death
//! (`WOLFSRC/WL_ACT2.C`, `WL_STATE.C`; OBC deltas in docs/re/ACTORS.md).
//!
//! STUB: interface fixed by the simulation skeleton; implementation pending.

use crate::data::Func;
use crate::world::{ObjId, World};

/// Runs an actor state function (think or action).
pub fn call(w: &mut World, id: ObjId, f: Func) {
    let _ = (w, id, f);
}

/// `SpawnStand(which, x, y, dir)` (ghidra 2390:04ca). `dir` is 0..3 (east, north, west, south).
pub fn spawn_stand(w: &mut World, which: u16, x: u16, y: u16, dir: u16) {
    let _ = (w, which, x, y, dir);
}

/// `SpawnPatrol(which, x, y, dir)` (ghidra 2390:0741). `hover` selects the
/// saucer's zero-speed variant (codes 270–273 vs 274–277).
pub fn spawn_patrol(w: &mut World, which: u16, x: u16, y: u16, dir: u16, hover: bool) {
    let _ = (w, which, x, y, dir, hover);
}

/// Special spawners for classes 13, 14, 16, 17, 22, 23 (ghidra 2390:0911 …).
pub fn spawn_special(w: &mut World, class: u16, x: u16, y: u16) {
    let _ = (w, class, x, y);
}

/// `SpawnFireActor(x, y)` (ghidra 30d6:0161). Returns false if refused.
pub fn spawn_fire_actor(w: &mut World, x: u16, y: u16) -> bool {
    let _ = (w, x, y);
    false
}

/// `DamageActor` (ghidra 1e43:174d).
pub fn damage_actor(w: &mut World, id: ObjId, damage: i16) {
    let _ = (w, id, damage);
}

/// `KillActor` (ghidra 1e43:1377).
pub fn kill_actor(w: &mut World, id: ObjId) {
    let _ = (w, id);
}

/// `CheckLine` (ghidra 1e43:1982): clear line from `id` to the player.
pub fn check_line(w: &World, id: ObjId) -> bool {
    let _ = (w, id);
    false
}

/// `IsBurnable(x, y)` (ghidra 30d6:0012).
pub fn is_burnable(w: &World, x: i32, y: i32) -> bool {
    let _ = (w, x, y);
    false
}

/// `Ignite`/`BurnTile(x, y)` (ghidra 30d6:0089).
pub fn ignite(w: &mut World, x: i32, y: i32) {
    let _ = (w, x, y);
}

/// Player grenade launcher (`SpawnGrenade`, ghidra 2f4d:0ff3).
pub fn spawn_grenade(w: &mut World, target: Option<ObjId>) {
    let _ = (w, target);
}

/// Player flame thrower (`SpawnFlame`, ghidra 30d6:0547).
pub fn spawn_player_flame(w: &mut World, target: Option<ObjId>) {
    let _ = (w, target);
}

/// Booby-trap blast one tile ahead of the player (ghidra 2390:0ae4).
pub fn spawn_explosion_ahead(w: &mut World) {
    let _ = w;
}
