//! Allies: spawning, Follow and Disperse/Attack modes, ally fire, the `F`,
//! `A` and `S` keys and body switching (docs/re/ALLIES.md, D-011, Q-017).
//!
//! Ports `COUNT.EXE` segment 2634 (ghidra `2634:000e`–`2634:1062`), which has
//! no Wolf3D counterpart, plus `InitAllies` (ghidra `30d6:0e5b`) and the mode
//! letter choice of `DrawAllyMode` (ghidra `30d6:08c0`). The Disperse/Attack
//! think is a copy of `T_Chase` (`WOLFSRC/WL_ACT2.C`) aimed at an enemy; its
//! direction choosers copy `SelectChaseDir`/`SelectDodgeDir` and its line test
//! copies `CheckLine` (`WOLFSRC/WL_STATE.C`).
//!
//! Follow mode runs the shared enemy `T_Chase` (ghidra `2390:1d2d`), and the
//! ally's own direction choosers call the shared `TryWalk` and `MoveObj`
//! (ghidra `1e43:01c3`, `1e43:10b1`). Those belong to `actors.rs`; this module
//! originally carried local equivalents marked `SHARED-AI` for the integrator to
//! replace.
//!
//! Every claim below comes from static disassembly (CONFIRMED = read
//! instruction by instruction); nothing has been traced in DOSBox-X yet.

use crate::actors;
use crate::data::{Func, StateId};
use crate::events::SimEvent;
use crate::world::{
    DoorAction, MAPSIZE, Obj, ObjId, Occ, TILEGLOBAL, TicCmd, World, class, dir, fl,
};

// ---------------------------------------------------------------------------
// Constants (docs/re/ALLIES.md §2)
// ---------------------------------------------------------------------------

/// `s_allystay1` (DS:0x2A26): the body left behind by `S`.
pub const S_ALLYSTAY1: StateId = 0x2A26;
/// `s_allypain1` (DS:0x2AA6), entered by `DamageActor` on odd hit points.
pub const S_ALLYPAIN1: StateId = 0x2AA6;
/// `s_allypain2` (DS:0x2AB6), entered by `DamageActor` on even hit points.
pub const S_ALLYPAIN2: StateId = 0x2AB6;
/// `s_allyshoot1` (DS:0x2AC6): action `T_AllyShoot`.
pub const S_ALLYSHOOT1: StateId = 0x2AC6;
/// `s_allychase1` (DS:0x2AE6): spawn state and `FirstSighting` state.
pub const S_ALLYCHASE1: StateId = 0x2AE6;
/// `s_allydie1` (DS:0x2B46), entered by `KillActor`.
pub const S_ALLYDIE1: StateId = 0x2B46;
/// `s_allydead` (DS:0x2BB6): the corpse (loops on itself).
pub const S_ALLYDEAD: StateId = 0x2BB6;

/// STAY flag. The original keeps it in bit 0 of byte +0x39 (ALLIES.md §2.1),
/// the high byte of the word at +0x38 that [`Obj::flags3`] holds, so it is bit 8
/// here. `GetNewActor`'s zero fill clears it. CONFIRMED (ghidra 2634:07c3).
pub const FL3_STAY: u16 = 0x0100;

/// `allymode` (DS:0x2A24) 0: Disperse/Attack, the mode at every level start.
pub const ALLYMODE_DISPERSE: i16 = 0;
/// `allymode` 1: Follow.
pub const ALLYMODE_FOLLOW: i16 = 1;
/// A third mode tested by `DrawAllyMode` but never set (vestigial).
const ALLYMODE_ATTACK: i16 = 0x1B;

/// `allyhp[gamestate.difficulty]` (DS:0x32EC). CONFIRMED (data).
const ALLYHP: [i16; 8] = [400, 400, 400, 400, 800, 800, 800, 400];
/// `allyfiresnds` (DS:0x2BCA), chosen by `US_RndT() % 3`. CONFIRMED (data).
const ALLYFIRESNDS: [u16; 3] = [47, 45, 46];
/// Ally death screams (DS:0x29A0), chosen by `US_RndT() % 5` in the class-5
/// case of `A_DeathScream` (ghidra 2390:0d74, in `actors.rs`). CONFIRMED (data).
pub const ALLYDEATHSNDS: [u16; 5] = [22, 23, 24, 25, 26];

/// Speed at spawn and after a switch (ghidra 2634:011b, 2634:1049).
const SPEED_SLOW: i32 = 0x400;
/// Speed set by `F` into Disperse/Attack (ghidra 2634:0080).
const SPEED_DISPERSE: i32 = 0xC00;
/// Speed set by `F` into Follow (ghidra 2634:00c0); also the clamp maximum.
const SPEED_FOLLOW: i32 = 0x1000;

/// `objlist` (DS:0xA623) and `sizeof(objtype)` (ACTORS.md §1). `ObjId` is the
/// `objlist` index, so `OBJLIST + id * OBJSIZE` is the original `objtype *`.
const OBJLIST: u16 = 0xA623;
const OBJSIZE: u16 = 0x42;
/// Word at DS:0x0004 ("Bo" of the Borland copyright): what `T_Ally` reads as
/// `obclass` when it dereferences its NULL target (ghidra 2634:09d7). CONFIRMED (data).
const DS_0004: u16 = 0x6F42;
/// `ICONARROWS` (plane-1 path arrows 90–97).
const ICONARROWS: u16 = 0x5A;
/// Plane-1 codes `CheckLine` treats as see-through (ACTORS.md §7.8).
const SEE_THROUGH: [u16; 3] = [0x69, 0x6B, 0x6A];

/// `GameMessage` strings (CONFIRMED, data).
const MSG_DISPERSE: &str = "DISPERSE AND ATTACK"; // DS:0x2BD2
const MSG_FOLLOW: &str = "FOLLOW PLAYER"; // DS:0x2BE6
const MSG_SWITCH: &str = "SWITCH"; // DS:0x0ADF
const MSG_STAYPUT: &str = "STAYPUT & SWITCH"; // DS:0x0AF1
const MSG_NO_ALLIES: &str = "NO ALLIES!"; // DS:0x0AE6

/// `DrawAllyMode` status-bar chunks (pics 25–28).
const CHUNK_MODE_ATTACK: u16 = 0x1C;
const CHUNK_MODE_FOLLOW: u16 = 0x1D;
const CHUNK_MODE_DISPERSE: u16 = 0x1E;
const CHUNK_MODE_NONE: u16 = 0x1F;

/// Ally keys carried in [`AlliesState::pending_keys`].
const KEY_F: u8 = 1;
const KEY_A: u8 = 2;
const KEY_S: u8 = 4;

// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------

/// Module state kept in `World::ally`. The ally count (DS:0x32FC) and mode
/// (DS:0x2A24) live in `World::allycount` / `World::allymode`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AlliesState {
    /// `switchstay` (DS:0x2BC6): 1 while `S` runs `SwitchAlly`, 0 for `A`.
    pub switchstay: i16,
    /// `allyfiring` (DS:0x2BC8): true only while an ally's shot is inside
    /// `DamageActor`. `GivePoints` (ghidra 208c:0614) returns at once and
    /// `KillActor` (ghidra 1e43:1615) does not count the kill while it is set.
    pub allyfiring: bool,
    /// `allyswitchidx` (DS:0x2BD1, byte): position in the `A`/`S` cycle.
    pub switchidx: u8,
    /// `allytarget` (DS:0xCDE5): **one** target shared by every ally, set when
    /// an ally decides to fire and read by `T_AllyShoot`. Never cleared.
    pub allytarget: Option<ObjId>,
    /// `swaptemp` (DS:0xCDA3): scratch object of `SwitchAlly`. Its `obclass`
    /// stays 0, which matters to `CopyActorState`.
    pub swaptemp: Obj,
    /// Status-bar chunk last drawn by `DrawAllyMode` (0 = never drawn). Kept
    /// because the original can show a stale letter: `InitAllies` draws before
    /// it resets the mode.
    pub mode_chunk: u16,
    /// Ally keys pressed but not yet handled (`KEY_*` bits). `CheckKeys` acts
    /// on one key per frame (F, then A, then S) and leaves the others in
    /// `Keyboard[]`, so a key that lost to a higher-priority one runs on the
    /// next call. OBCR emulation of a held key; not original state.
    pub pending_keys: u8,
}

// ---------------------------------------------------------------------------
// Public interface
// ---------------------------------------------------------------------------

/// Runs an ally state function (`T_Ally` think or `T_AllyShoot` action).
pub fn call(w: &mut World, id: ObjId, f: Func) {
    match f {
        Func::TAlly => t_ally(w, id),
        Func::AAlly => t_ally_shoot(w, id),
        other => {
            w.emit(SimEvent::Fault(format!("allies::call: {other:?} is not an ally function")))
        }
    }
}

/// Places an ally from plane-1 codes 224–227; `dir` 0–3 = east, north, west,
/// south.
///
/// Port of `SpawnAlly` (ghidra 2634:00e4; CONFIRMED, ALLIES.md §1.2). No Wolf3D
/// counterpart; the tile shift is Wolf3D's `SpawnPatrol` shift
/// (`WOLFSRC/WL_ACT2.C`) with an OBC revert test.
#[doc(alias = "SpawnAlly")]
pub fn spawn_ally(w: &mut World, x: u16, y: u16, dir: u16) {
    // SpawnNewObj leaves `new` NULL when the list is full; SpawnAlly then does nothing.
    let Some(id) = w.spawn_new_obj(x, y, S_ALLYCHASE1) else {
        return;
    };
    let difficulty = w.gamestate.difficulty;
    // allyhp[difficulty]: the original indexes without a bound check.
    let hp = usize::try_from(difficulty).ok().and_then(|i| ALLYHP.get(i)).copied().unwrap_or(400);
    {
        let o = w.obj_mut(id);
        o.obclass = class::ALLY;
        o.dir = dir.wrapping_mul(2); // 4-way code to 8-way dirtype
        o.speed = SPEED_SLOW;
        o.hitpoints = hp;
        o.distance = TILEGLOBAL; // copied from DS:0x0D1A
        o.flags |= fl::SHOOTABLE;
        o.flags |= fl::AMBUSH;
        if difficulty < 4 {
            o.flags2 |= fl::NOBLOCK; // the player walks through allies
        }
        o.active = 1;
    }
    // `new->state->rotate = 1` writes the state record itself; it already is 1.

    // SpawnPatrol tile shift, kept only if the next tile already holds an
    // actor pointer or a door code >= 0x8C (empty reads 0). x/y are not moved.
    let (sx, sy) = (w.obj(id).tilex, w.obj(id).tiley);
    w.set_actorat(tile_i32(sx), tile_i32(sy), Occ::Empty);
    let (mut tx, mut ty) = (sx, sy);
    match dir {
        0 => tx = tx.wrapping_add(1),
        1 => ty = ty.wrapping_sub(1),
        2 => tx = tx.wrapping_sub(1),
        3 => ty = ty.wrapping_add(1),
        _ => {}
    }
    if actorat_word(w.actorat(tile_i32(tx), tile_i32(ty))) < 0x8C {
        tx = sx;
        ty = sy;
    }
    let o = w.obj_mut(id);
    o.tilex = tx;
    o.tiley = ty;
    w.set_actorat(tile_i32(tx), tile_i32(ty), Occ::Obj(id));
}

/// Counts the living allies and resets the mode; called at every `PlayLoop`
/// entry (level start and after loading, ghidra 1942:14e1).
///
/// Port of `InitAllies` (ghidra 30d6:0e5b; CONFIRMED, ALLIES.md §1.3).
#[doc(alias = "InitAllies")]
pub fn init_allies(w: &mut World) {
    let mut count: u8 = 0; // allycount is a byte (DS:0x32FC)
    w.ally.switchidx = 0;
    if w.gamestate.difficulty < 4 {
        for o in after_player(w) {
            let ob = w.obj(o);
            if ob.obclass == class::ALLY && ob.active != 0 && ob.flags & fl::NONMARK == 0 {
                count = count.wrapping_add(1);
            }
        }
    }
    w.allycount = i16::from(count);
    if w.gamestate.difficulty < 4 && count > 5 {
        quit(w, 1802, "InitAllies: more than 5 allies");
    }
    // Drawn with the previous level's mode: the reset below comes after it.
    draw_ally_mode(w);
    // FUN_30d6_1c02 follows here: UNKNOWN role, failed to decompile (ALLIES.md §11.6).
    w.allymode = ALLYMODE_DISPERSE;
    w.ally.pending_keys = 0;
}

/// Ally keys from `CheckKeys` (ghidra 1942:04ea, CONFIRMED, ALLIES.md §5.1):
/// `F` toggles the mode, `A` switches bodies, `S` switches and freezes the body
/// left behind. One key acts per call, in the original's order F, A, S.
///
/// The caller must apply the original's outer conditions: not paused
/// (DS:0x8F20), no Alt+Shift, and none of the keys `CheckKeys` tests earlier
/// (H, C and the function keys) handled this frame.
#[doc(alias = "CheckKeys")]
pub fn handle_keys(w: &mut World, cmd: &TicCmd) {
    let mut keys = w.ally.pending_keys;
    if cmd.ally_mode {
        keys |= KEY_F;
    }
    if cmd.ally_switch {
        keys |= KEY_A;
    }
    if cmd.ally_stay {
        keys |= KEY_S;
    }
    if w.gamestate.difficulty >= 4 {
        // A and S are never acted on in network games.
        keys &= !(KEY_A | KEY_S);
    }

    if keys & KEY_F != 0 {
        w.ally.pending_keys = keys & !KEY_F;
        toggle_ally_mode(w);
        return;
    }
    if keys & KEY_A != 0 {
        w.ally.pending_keys = keys & !KEY_A;
        w.ally.switchstay = 0;
        if switch_ally(w) {
            w.emit(SimEvent::Message(MSG_SWITCH.into()));
            // TODO(M9.4): ClearScreenSplats (ghidra 30d6:10c5) needs a SimEvent.
            // `S` does not clear the splats (CONFIRMED inconsistency).
        } else {
            w.emit(SimEvent::Message(MSG_NO_ALLIES.into()));
        }
        return;
    }
    if keys & KEY_S != 0 {
        w.ally.pending_keys = keys & !KEY_S;
        w.ally.switchstay = 1;
        if switch_ally(w) {
            w.emit(SimEvent::Message(MSG_STAYPUT.into()));
        } else {
            w.emit(SimEvent::Message(MSG_NO_ALLIES.into()));
            w.ally.switchstay = 0;
        }
        return;
    }
    w.ally.pending_keys = 0;
}

/// The ally an enemy shoots instead of the player, if any.
///
/// Port of `FindNearestVisibleActor` (ghidra 2634:036e; CONFIRMED, ALLIES.md
/// §4.5) followed by the class test of its only caller, `T_ShootAlly`
/// (ghidra 2390:26b9 at 26d4–26dc): the result is returned only when it is an
/// ally. Quirks kept:
/// - candidates are actors after the player with another class than `enemy`,
///   `FL_SHOOTABLE` and `FL_VISABLE` (set by the renderer: only actors on the
///   player's screen qualify);
/// - "distance" is `|Δx| + |Δy|` over the **low 16 bits** of `x`/`y` (the
///   sub-tile fraction), so "nearest" is effectively arbitrary; ties keep the
///   first in list order.
#[doc(alias = "FindNearestVisibleActor")]
#[doc(alias = "FUN_2634_036e")]
pub fn enemy_target(w: &mut World, enemy: ObjId) -> Option<ObjId> {
    let found = find_nearest_visible_actor(w, enemy)?;
    (w.obj(found).obclass == class::ALLY).then_some(found)
}

/// Line of sight from an ally to an enemy, as `T_ShootAlly` tests it:
/// `CheckLineBetween(ally, enemy)` (ghidra 2634:0412, see
/// [`check_line_between`] for its OBC quirks).
#[doc(alias = "CheckLineBetween")]
#[doc(alias = "FUN_2634_0412")]
pub fn can_see(w: &mut World, ally: ObjId, enemy: ObjId) -> bool {
    check_line_between(w, ally, enemy)
}

// ---------------------------------------------------------------------------
// F: ToggleAllyMode
// ---------------------------------------------------------------------------

/// Port of `ToggleAllyMode` (ghidra 2634:000e; CONFIRMED, ALLIES.md §5.2).
/// The message is issued once per living ally; with none the mode still flips.
#[doc(alias = "ToggleAllyMode")]
fn toggle_ally_mode(w: &mut World) {
    if w.gamestate.difficulty >= 4 {
        return;
    }
    w.allymode = i16::from(w.allymode == 0);
    for o in after_player(w) {
        let ob = w.obj(o);
        if ob.obclass != class::ALLY || ob.flags & fl::SHOOTABLE == 0 {
            continue;
        }
        match w.allymode {
            ALLYMODE_DISPERSE => {
                w.new_state(o, S_ALLYCHASE1);
                let ob = w.obj_mut(o);
                ob.flags = 0; // clears ATTACKMODE, FIRSTATTACK, VISABLE
                ob.flags |= fl::SHOOTABLE;
                ob.flags |= fl::AMBUSH;
                ob.flags2 |= fl::NOBLOCK;
                ob.speed = SPEED_DISPERSE;
                w.emit(SimEvent::Message(MSG_DISPERSE.into()));
            }
            ALLYMODE_FOLLOW => {
                // CheckSight(o) (ghidra 1e43:1d35) is called here; its result
                // is ignored and it has no side effects.
                w.new_state(o, S_ALLYCHASE1);
                let ob = w.obj_mut(o);
                ob.flags = 0;
                ob.flags |= fl::SHOOTABLE;
                ob.flags |= fl::AMBUSH;
                ob.flags2 |= fl::NOBLOCK;
                ob.flags3 &= !FL3_STAY; // only a toggle into Follow clears STAY
                ob.speed = SPEED_FOLLOW;
                w.emit(SimEvent::Message(MSG_FOLLOW.into()));
            }
            _ => {}
        }
    }
    draw_ally_mode(w);
}

/// Chooses the status-bar mode letter. Port of `DrawAllyMode`
/// (ghidra 30d6:08c0; CONFIRMED): blank with no allies, "D" in mode 0, "F" in
/// mode 1, "A" in the unused mode 0x1B, nothing drawn otherwise.
#[doc(alias = "DrawAllyMode")]
fn draw_ally_mode(w: &mut World) {
    let chunk = if w.allycount as u8 == 0 {
        Some(CHUNK_MODE_NONE)
    } else {
        match w.allymode {
            ALLYMODE_DISPERSE => Some(CHUNK_MODE_DISPERSE),
            ALLYMODE_FOLLOW => Some(CHUNK_MODE_FOLLOW),
            ALLYMODE_ATTACK => Some(CHUNK_MODE_ATTACK),
            _ => None,
        }
    };
    if let Some(c) = chunk {
        w.ally.mode_chunk = c;
        w.emit(SimEvent::Hud);
    }
}

// ---------------------------------------------------------------------------
// A / S: SwitchAlly
// ---------------------------------------------------------------------------

/// Swaps the player with the next living ally. Returns false ("NO ALLIES!")
/// in network games or when no living ally exists.
///
/// Port of `SwitchAlly` (ghidra 2634:0e35; CONFIRMED, ALLIES.md §5.3).
/// `actorat`, `areabyplayer` and the camera are not touched; the left body
/// keeps its state, flags and `distance`.
#[doc(alias = "SwitchAlly")]
fn switch_ally(w: &mut World) -> bool {
    if w.gamestate.difficulty >= 4 {
        return false;
    }
    let count = w.allycount as u8;
    if w.ally.switchidx == count {
        w.ally.switchidx = 0;
    }
    if count == 1 {
        w.ally.switchidx = 0;
    }
    let mut k: u8 = 0;
    let mut found = None;
    for o in after_player(w) {
        if is_living_ally(w.obj(o)) {
            k = k.wrapping_add(1);
            if k > w.ally.switchidx {
                found = Some(o);
                break;
            }
        }
    }
    let o = match found {
        // With the debug flag (DS:0x0954) set, FUN_2634_0fde(o) runs here: an empty stub.
        Some(o) => o,
        None => {
            // Wrapped past the last living ally: take the first one (the
            // original also zeroes its counter here before setting it to 1).
            w.ally.switchidx = 0;
            let Some(o) = after_player(w).into_iter().find(|&o| is_living_ally(w.obj(o))) else {
                return false;
            };
            k = 1;
            // DebugMessage("BAD SWATCH") (DS:0x2C15): shown only with the debug flag.
            o
        }
    };

    let health = w.gamestate.health;
    let player = w.player;
    let mut swaptemp = w.ally.swaptemp;
    let ally = *w.obj(o);
    copy_actor_state(&mut swaptemp, &ally, health);
    let plr = *w.obj(player);
    copy_actor_state(w.obj_mut(o), &plr, health);
    copy_actor_state(w.obj_mut(player), &swaptemp, health);
    w.ally.swaptemp = swaptemp;

    // The original sets STAY and s_allystay1 twice, around the health copy.
    if w.ally.switchstay != 0 {
        w.obj_mut(o).flags3 |= FL3_STAY;
        w.new_state(o, S_ALLYSTAY1);
    }
    w.gamestate.health = swaptemp.hitpoints;
    if w.ally.switchstay != 0 {
        w.new_state(o, S_ALLYSTAY1);
        w.obj_mut(o).flags3 |= FL3_STAY;
    }
    w.emit(SimEvent::Hud); // DrawHealth (ghidra 30d6:0b4a)
    w.ally.switchidx = k;
    true
}

/// Port of `CopyActorState(dst, src)` (ghidra 2634:0fe3; CONFIRMED,
/// ALLIES.md §5.4). Copies position, tile, area, view x, angle, direction and
/// hit points (the player's come from `gamestate.health`), caps hit points at
/// 999, sets speed 0x400 and clears STAY on `dst`.
#[doc(alias = "CopyActorState")]
fn copy_actor_state(dst: &mut Obj, src: &Obj, health: i16) {
    dst.dir = src.dir;
    dst.x = src.x;
    dst.y = src.y;
    dst.tilex = src.tilex;
    dst.tiley = src.tiley;
    dst.hitpoints = if src.obclass == class::PLAYER { health } else { src.hitpoints };
    if dst.hitpoints > 999 {
        dst.hitpoints = 999;
    }
    dst.areanumber = src.areanumber;
    dst.angle = src.angle;
    dst.speed = SPEED_SLOW;
    dst.viewx = src.viewx;
    dst.angle = src.angle;
    dst.flags3 &= !FL3_STAY;
}

/// `obclass == 5 && (flags & FL_SHOOTABLE) && !(flags & FL_NONMARK)`, the
/// living-ally test of `SwitchAlly`.
fn is_living_ally(o: &Obj) -> bool {
    o.obclass == class::ALLY && o.flags & fl::SHOOTABLE != 0 && o.flags & fl::NONMARK == 0
}

// ---------------------------------------------------------------------------
// Think: T_Ally
// ---------------------------------------------------------------------------

/// Ally think for the chase and stay states.
///
/// Port of `T_Ally` (ghidra 2634:07b8; CONFIRMED, ALLIES.md §3.1, §4.1).
/// STAY returns at once. Follow runs the shared `T_Chase`. Disperse/Attack is
/// `T_Chase` (`WOLFSRC/WL_ACT2.C`) with the first enemy in list order that the
/// ally has a clear line to (not the nearest) in place of the player.
#[doc(alias = "T_Ally")]
fn t_ally(w: &mut World, id: ObjId) {
    if w.obj(id).flags3 & FL3_STAY != 0 {
        return;
    }
    ally_clamp_speed(w, id);
    if w.gamestate.difficulty >= 4 {
        return;
    }
    if w.allymode == ALLYMODE_FOLLOW {
        t_chase(w, id);
        return;
    }

    let mut dodge = false;
    let mut target = None;
    for o in after_player(w) {
        let c = w.obj(o).obclass;
        if c == class::PLAYER || c == class::FIRE || c == class::ALLY {
            continue;
        }
        // The line test runs before the shootable test: corpses cost a ray cast.
        if check_line_between(w, id, o) && w.obj(o).flags & fl::SHOOTABLE != 0 {
            target = Some(o);
            break;
        }
    }
    if let Some(t) = target {
        let ob = *w.obj(id);
        let dist = tile_dist(&ob, w.obj(t));
        let chance = attack_chance(w, &ob, dist);
        if w.rnd() < chance {
            // US_RndT: T_Ally fire decision (2634:08b8)
            set_ally_target(w, t);
            w.new_state(id, S_ALLYSHOOT1);
            return;
        }
        dodge = true;
    }

    if w.obj(id).dir == dir::NODIR {
        if dodge {
            halve_speed(w, id);
            select_dodge_dir_to(w, id, target);
            double_speed(w, id);
        } else {
            select_chase_dir_to(w, id, target);
        }
        if w.obj(id).dir == dir::NODIR {
            return;
        }
    }

    let mut mv = w.obj(id).speed.wrapping_mul(i32::from(w.tics as u16));
    while mv != 0 {
        if !wait_for_door(w, id) {
            return;
        }
        let distance = w.obj(id).distance;
        if distance > mv {
            move_obj(w, id, mv);
            return;
        }
        snap_to_tile_centre(w.obj_mut(id));
        mv = mv.wrapping_sub(distance);

        // `target->obclass == 5` with a NULL target reads DS:0x0004 (never 5).
        let tclass = target.map_or(DS_0004, |t| w.obj(t).obclass);
        if tclass != class::ALLY && dodge {
            halve_speed(w, id);
            select_dodge_dir_to(w, id, target);
            double_speed(w, id);
        } else {
            select_chase_dir_to(w, id, target);
        }
        if w.obj(id).dir == dir::NODIR {
            return;
        }
    }
}

/// Clamps the speed to [0x400, 0x1000]. Port of `AllyClampSpeed`
/// (ghidra 2634:0a24; CONFIRMED, ALLIES.md §3.4). Its messages ("Ally stuck
/// throttle", "Ally stalled") go through `DebugMessage` and print only with
/// the debug flag; they are not emitted.
#[doc(alias = "AllyClampSpeed")]
fn ally_clamp_speed(w: &mut World, id: ObjId) {
    // `if (speed > 0x1000) speed = 0x1000; if (speed < 0x400) speed = 0x400;`
    let o = w.obj_mut(id);
    o.speed = o.speed.clamp(SPEED_SLOW, SPEED_FOLLOW);
}

/// Port of `SetAllyTarget` (ghidra 2634:0209; CONFIRMED).
#[doc(alias = "SetAllyTarget")]
fn set_ally_target(w: &mut World, t: ObjId) {
    w.ally.allytarget = Some(t);
}

/// The door-wait block of `T_Chase` as `T_Ally` has it (ghidra 2634:093c).
/// Returns false when the ally must keep waiting.
fn wait_for_door(w: &mut World, id: ObjId) -> bool {
    let distance = w.obj(id).distance;
    if distance >= 0 {
        return true;
    }
    // OpenDoor(-distance-1) on the low word, then doorobjlist[...].action.
    let door = (distance as i16).wrapping_neg().wrapping_sub(1);
    let idx = usize::try_from(door).unwrap_or(usize::MAX);
    w.open_door(idx);
    if w.doors.get(idx).map(|d| d.action) != Some(DoorAction::Open) {
        return false;
    }
    w.obj_mut(id).distance = TILEGLOBAL; // the door is open now
    true
}

// ---------------------------------------------------------------------------
// Direction choosers: SelectChaseDirTo / SelectDodgeDirTo
// ---------------------------------------------------------------------------

/// `SelectChaseDir` toward `target` instead of the player.
///
/// Port of ghidra 2634:0c48 (CONFIRMED asm; ALLIES.md §4.2), a copy of
/// `SelectChaseDir` (`WOLFSRC/WL_STATE.C`). Deltas: with no target, a path
/// arrow under the ally is tried first and Δ = (+1, +1) (drift east and
/// south); `opposite[player->dir]` is a last resort before `nodir`.
#[doc(alias = "SelectChaseDirTo")]
fn select_chase_dir_to(w: &mut World, id: ObjId, target: Option<ObjId>) {
    let ob = *w.obj(id);
    let olddir = ob.dir;
    let turnaround = opposite(olddir);
    let (deltax, deltay) = match target {
        Some(t) => {
            let t = w.obj(t);
            (
                (t.tilex as i16).wrapping_sub(ob.tilex as i16),
                (t.tiley as i16).wrapping_sub(ob.tiley as i16),
            )
        }
        None => {
            if try_path_arrow(w, id) {
                return;
            }
            (1, 1)
        }
    };

    let mut d1 = dir::NODIR;
    let mut d2 = dir::NODIR;
    if deltax > 0 {
        d1 = dir::EAST;
    } else if deltax < 0 {
        d1 = dir::WEST;
    }
    if deltay > 0 {
        d2 = dir::SOUTH;
    } else if deltay < 0 {
        d2 = dir::NORTH;
    }
    if deltay.wrapping_abs() > deltax.wrapping_abs() {
        std::mem::swap(&mut d1, &mut d2);
    }
    if d1 == turnaround {
        d1 = dir::NODIR;
    }
    if d2 == turnaround {
        d2 = dir::NODIR;
    }
    if d1 != dir::NODIR && walk(w, id, d1) {
        return;
    }
    if d2 != dir::NODIR && walk(w, id, d2) {
        return;
    }
    // No direct path: pick another direction.
    if olddir != dir::NODIR && walk(w, id, olddir) {
        return;
    }
    // US_RndT: SelectChaseDirTo search order (2634:0d94)
    if w.rnd() > 128 {
        for tdir in dir::NORTH..=dir::WEST {
            if tdir != turnaround && walk(w, id, tdir) {
                return;
            }
        }
    } else {
        for tdir in (dir::NORTH..=dir::WEST).rev() {
            if tdir != turnaround && walk(w, id, tdir) {
                return;
            }
        }
    }
    if turnaround != dir::NODIR && walk(w, id, turnaround) {
        return;
    }
    // OBC: last resort, away from the way the player faces.
    let pdir = opposite(w.plr().dir);
    if walk(w, id, pdir) {
        return;
    }
    w.obj_mut(id).dir = dir::NODIR;
}

/// `SelectDodgeDir` toward `target` instead of the player.
///
/// Port of ghidra 2634:0a79 (CONFIRMED asm; ALLIES.md §4.2), a copy of
/// `SelectDodgeDir` (`WOLFSRC/WL_STATE.C`) without the shared copy's
/// 1-in-5 gate. Deltas as [`select_chase_dir_to`].
#[doc(alias = "SelectDodgeDirTo")]
fn select_dodge_dir_to(w: &mut World, id: ObjId, target: Option<ObjId>) {
    let turnaround = if w.obj(id).flags & fl::FIRSTATTACK != 0 {
        // Turning around is only allowed right after noticing the target.
        w.obj_mut(id).flags &= !fl::FIRSTATTACK;
        dir::NODIR
    } else {
        opposite(w.obj(id).dir)
    };
    let ob = *w.obj(id);
    let (deltax, deltay) = match target {
        Some(t) => {
            let t = w.obj(t);
            (
                (t.tilex as i16).wrapping_sub(ob.tilex as i16),
                (t.tiley as i16).wrapping_sub(ob.tiley as i16),
            )
        }
        None => {
            if try_path_arrow(w, id) {
                return;
            }
            (1, 1)
        }
    };

    // Five choices in order of preference: the diagonal, then the cardinals.
    let mut dirtry = [dir::NODIR; 5];
    if deltax > 0 {
        dirtry[1] = dir::EAST;
        dirtry[3] = dir::WEST;
    } else {
        dirtry[1] = dir::WEST;
        dirtry[3] = dir::EAST;
    }
    if deltay > 0 {
        dirtry[2] = dir::SOUTH;
        dirtry[4] = dir::NORTH;
    } else {
        dirtry[2] = dir::NORTH;
        dirtry[4] = dir::SOUTH;
    }
    let absdx = deltax.wrapping_abs() as u16;
    let absdy = deltay.wrapping_abs() as u16;
    if absdx > absdy {
        dirtry.swap(1, 2);
        dirtry.swap(3, 4);
    }
    // US_RndT: SelectDodgeDirTo randomisation (2634:0b7a)
    if w.rnd() < 128 {
        dirtry.swap(1, 2);
        dirtry.swap(3, 4);
    }
    dirtry[0] = dir::diagonal(dirtry[1], dirtry[2]);

    for &d in &dirtry {
        if d == dir::NODIR || d == turnaround {
            continue;
        }
        if walk(w, id, d) {
            return;
        }
    }
    if turnaround != dir::NODIR && walk(w, id, turnaround) {
        return;
    }
    let pdir = opposite(w.plr().dir);
    if walk(w, id, pdir) {
        return;
    }
    w.obj_mut(id).dir = dir::NODIR;
}

/// The no-target branch shared by both choosers: a plane-1 path arrow
/// (`ICONARROWS` + dir) under the ally gives the direction.
fn try_path_arrow(w: &mut World, id: ObjId) -> bool {
    let ob = *w.obj(id);
    let code = w.mapspot(tile_i32(ob.tilex), tile_i32(ob.tiley), 1).wrapping_sub(ICONARROWS);
    code < 8 && walk(w, id, code)
}

/// `ob->dir = d; TryWalk(ob)`.
fn walk(w: &mut World, id: ObjId, d: u16) -> bool {
    w.obj_mut(id).dir = d;
    try_walk(w, id)
}

// ---------------------------------------------------------------------------
// Line of sight: CheckLineBetween
// ---------------------------------------------------------------------------

/// Clear line between two actors.
///
/// Port of `CheckLineBetween(a, b)` (ghidra 2634:0412; CONFIRMED asm), Wolf3D's
/// `CheckLine` (`WOLFSRC/WL_STATE.C`) with `a` in place of the player and `b`
/// as the looking actor. OBC deltas:
/// - `x2`/`y2` are the **low words** of `a->x`/`a->y` (the 1/65536 tile
///   fraction), where the shared `CheckLine` (ghidra 1e43:1982) uses
///   `plux`/`pluy` (1/256 tile units). The slope is therefore garbage: with `a`
///   at a tile centre the ray runs at about 45° toward +y (x pass) or +x
///   (y pass) whatever the true direction. Kept as is.
/// - `deltafrac == 0` becomes 1 (the shared `CheckLine` quits with 305/306).
/// - Tiles whose plane-1 value is 0x69–0x6B are see-through.
/// - Doors block by `doorposition`, as in Wolf3D. Actors never block.
///
/// Callers: `T_Ally(ob)` tests `(ally, candidate)`, `T_AllyShoot` tests
/// `(allytarget, ally)` and `T_ShootAlly` tests `(ally, enemy)`.
#[doc(alias = "CheckLineBetween")]
fn check_line_between(w: &World, a: ObjId, b: ObjId) -> bool {
    let (oa, ob) = (w.obj(a), w.obj(b));
    let x1 = (ob.x >> 8) as i16; // LXRSH@ 8, low word
    let y1 = (ob.y >> 8) as i16;
    let xt1 = x1 >> 8;
    let yt1 = y1 >> 8;
    let x2 = oa.x as i16; // OBC: low word of a->x, not a->x >> 8
    let y2 = oa.y as i16;
    let xt2 = oa.tilex as i16;
    let yt2 = oa.tiley as i16;

    // x pass: step across each vertical grid line.
    if xt2.wrapping_sub(xt1).wrapping_abs() > 0 {
        let (partial, xstep) = if xt2 > xt1 { (0x100 - (x1 & 0xFF), 1) } else { (x1 & 0xFF, -1) };
        let mut deltafrac = x2.wrapping_sub(x1).wrapping_abs();
        let delta = y2.wrapping_sub(y1);
        if deltafrac == 0 {
            deltafrac = 1;
        }
        let ystep = line_step(delta, deltafrac);
        let mut yfrac = y1.wrapping_add(((i32::from(ystep) * i32::from(partial)) >> 8) as i16);
        let mut x = xt1.wrapping_add(xstep);
        let xt2 = xt2.wrapping_add(xstep);
        loop {
            let y = yfrac >> 8;
            yfrac = yfrac.wrapping_add(ystep);
            let value = tilemap_flat(w, x, y);
            let see_through = plane1_flat(w, y, x).is_some_and(|v| SEE_THROUGH.contains(&v));
            x = x.wrapping_add(xstep);
            if !see_through && value != 0 && !door_lets_through(w, value, yfrac, ystep) {
                return false;
            }
            if x == xt2 {
                break;
            }
        }
    }

    // y pass: step across each horizontal grid line.
    if yt2.wrapping_sub(yt1).wrapping_abs() > 0 {
        let (partial, ystep) = if yt2 > yt1 { (0x100 - (y1 & 0xFF), 1) } else { (y1 & 0xFF, -1) };
        let mut deltafrac = y2.wrapping_sub(y1).wrapping_abs();
        if deltafrac == 0 {
            deltafrac = 1;
        }
        let delta = x2.wrapping_sub(x1);
        let xstep = line_step(delta, deltafrac);
        let mut xfrac = x1.wrapping_add(((i32::from(xstep) * i32::from(partial)) >> 8) as i16);
        let mut y = yt1.wrapping_add(ystep);
        let yt2 = yt2.wrapping_add(ystep);
        loop {
            let x = xfrac >> 8;
            xfrac = xfrac.wrapping_add(xstep);
            let value = tilemap_flat(w, x, y);
            let see_through = plane1_flat(w, y, x).is_some_and(|v| SEE_THROUGH.contains(&v));
            y = y.wrapping_add(ystep);
            if !see_through && value != 0 && !door_lets_through(w, value, xfrac, xstep) {
                return false;
            }
            if y == yt2 {
                break;
            }
        }
    }
    true
}

/// `ltemp = ((long)delta << 8) / deltafrac` (F_LDIV@, truncating), clamped to
/// ±0x7FFF.
fn line_step(delta: i16, deltafrac: i16) -> i16 {
    let ltemp = (i32::from(delta) << 8) / i32::from(deltafrac);
    if ltemp > 0x7FFF {
        0x7FFF
    } else if ltemp < -0x7FFF {
        -0x7FFF
    } else {
        ltemp as i16
    }
}

/// The non-zero tile test of `CheckLine`: walls (< 0x80, > 0x100) block; a
/// door blocks when `intercept = frac - step/2` lies past its open edge
/// (unsigned compare).
fn door_lets_through(w: &World, value: u16, frac: i16, step: i16) -> bool {
    if !(0x80..=0x100).contains(&value) {
        return false;
    }
    let door = usize::from(value & 0xFF7F);
    let intercept = frac.wrapping_sub(step / 2) as u16;
    // doorposition[] has MAXDOORS entries; unused ones read 0 (closed).
    let position = w.doors.get(door).map_or(0, |d| d.position);
    intercept <= position
}

/// `tilemap[x][y]` read as the original does: one flat byte array indexed
/// `x*64 + y`, so a row overflow reads the neighbouring column.
fn tilemap_flat(w: &World, x: i16, y: i16) -> u16 {
    let idx = i32::from(x) * MAPSIZE as i32 + i32::from(y);
    match usize::try_from(idx).ok().and_then(|i| w.tilemap.get(i)) {
        Some(&v) => u16::from(v),
        // HYPOTHESIS(Q-017): outside tilemap the original reads unrelated
        // DGROUP bytes; treat as a wall (line blocked).
        None => 1,
    }
}

/// Plane-1 word at `farmapylookup[y] + x`, as the original indexes it.
fn plane1_flat(w: &World, y: i16, x: i16) -> Option<u16> {
    if !(0..MAPSIZE as i16).contains(&y) {
        // HYPOTHESIS(Q-017): farmapylookup[y] is out of bounds and the
        // original reads an unknown word; treat as not see-through.
        return None;
    }
    let idx = i32::from(y) * MAPSIZE as i32 + i32::from(x);
    // Same fallback when the flat index leaves the plane.
    usize::try_from(idx).ok().and_then(|i| w.plane1.get(i)).copied()
}

// ---------------------------------------------------------------------------
// Action: T_AllyShoot
// ---------------------------------------------------------------------------

/// Ally fire, the action of `s_allyshoot1`.
///
/// Port of `T_AllyShoot` (ghidra 2634:0214; CONFIRMED asm, ALLIES.md §4.4).
/// RNG calls: 1 (sound, even with no target), then 1 (hit roll), +1 on a hit.
/// Quirks kept:
/// - the fire sound plays before the target is checked;
/// - the shooter turns: `dir = opposite[target->dir]` (movement, not only facing);
/// - `DamageActor` runs on a miss too, with `DI` unassigned. `DI` still holds
///   `DoActor`'s actor pointer (ghidra 1942:10fd, kept across the action call at
///   1942:1229; every callee in between preserves `DI`), so a miss deals
///   `2 × (DS address of the shooting ally)`: 19,658–32,594 for `objlist`
///   slots 1–99, doubled again (to a negative "heal") by `DamageActor` when the
///   target is not in attack mode. The value is fully determined by the code;
///   the in-game effect still awaits a trace (Q-017, ALLIES.md §11.1);
/// - disguised soldiers (classes 8–11) not yet alerted are woken *before*
///   the damage;
/// - `allyfiring` suppresses score and kill count inside `DamageActor`.
#[doc(alias = "T_AllyShoot")]
fn t_ally_shoot(w: &mut World, id: ObjId) {
    if w.gamestate.difficulty >= 4 {
        actors::call(w, id, Func::TShoot); // T_Shoot (ghidra 2390:2506)
        return;
    }
    // US_RndT: ally fire sound (2634:0244)
    let snd = ALLYFIRESNDS[(w.rnd() % 3) as usize];
    let (x, y) = (w.obj(id).x, w.obj(id).y);
    w.play_sound_at(snd, x, y);

    let Some(t) = w.ally.allytarget else {
        return;
    };
    let tclass = w.obj(t).obclass;
    if tclass == class::PLAYER || tclass == class::ALLY {
        quit(w, 1602, "T_AllyShoot: target is the player or an ally");
        return;
    }
    if !check_line_between(w, t, id) {
        return;
    }
    let dist = tile_dist(w.obj(id), w.obj(t));
    let tdir = w.obj(t).dir;
    w.obj_mut(id).dir = opposite(tdir);

    // DI on a miss: the shooter's objtype pointer (see above).
    let mut di = objlist_addr(id);
    // US_RndT: ally hit roll, fixed 50% (2634:02de)
    if w.rnd() < 128 {
        // US_RndT: ally hit damage (2634:02ee/0300/030c)
        let damage = if dist < 2 {
            w.rnd() >> 2
        } else if dist < 4 {
            w.rnd() >> 3
        } else {
            w.rnd() >> 4
        };
        di = damage as u16;
    }
    let t_ob = w.obj(t);
    if t_ob.obclass.wrapping_sub(8) <= 3 && t_ob.flags & (fl::ATTACKMODE | fl::FIRSTATTACK) == 0 {
        first_sighting(w, t);
    }
    w.ally.allyfiring = true;
    actors::damage_actor(w, t, di.wrapping_mul(2) as i16);
    w.ally.allyfiring = false;
}

/// The `DS` offset of `objlist[id]`, i.e. the original's `objtype *`.
fn objlist_addr(id: ObjId) -> u16 {
    OBJLIST.wrapping_add(u16::from(id).wrapping_mul(OBJSIZE))
}

/// The `actorat` word the original compares: 0, a wall or door code, or an
/// object pointer (≥ 0xA623).
fn actorat_word(occ: Occ) -> u16 {
    match occ {
        Occ::Empty => 0,
        Occ::Mark(v) => v,
        Occ::Obj(id) => objlist_addr(id),
    }
}

// ---------------------------------------------------------------------------
// FindNearestVisibleActor
// ---------------------------------------------------------------------------

/// Port of ghidra 2634:036e (CONFIRMED asm `2634:0392`–`0409`): see
/// [`enemy_target`].
fn find_nearest_visible_actor(w: &World, enemy: ObjId) -> Option<ObjId> {
    let e = *w.obj(enemy);
    let mut best = i32::MAX; // 0x7FFF:0xFFFF
    let mut found = None;
    for o in after_player(w) {
        let c = w.obj(o);
        if c.obclass == e.obclass || c.flags & fl::SHOOTABLE == 0 || c.flags & fl::VISABLE == 0 {
            continue;
        }
        // 16-bit differences of the low words, abs (cwd/xor/sub), then summed as longs.
        let ax = i32::from((c.x as u16).wrapping_sub(e.x as u16).cast_signed().wrapping_abs());
        let ay = i32::from((c.y as u16).wrapping_sub(e.y as u16).cast_signed().wrapping_abs());
        let d = ax + ay;
        if d < best {
            best = d;
            found = Some(o);
        }
    }
    found
}

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

/// Objects after the player in list order (`for (o = player->next; o; o = o->next)`).
fn after_player(w: &World) -> Vec<ObjId> {
    let mut ids = w.obj_ids();
    if !ids.is_empty() {
        ids.remove(0);
    }
    ids
}

/// `max(abs(a.tilex - b.tilex), abs(a.tiley - b.tiley))` in 16 bits.
fn tile_dist(a: &Obj, b: &Obj) -> i16 {
    let dx = (a.tilex as i16).wrapping_sub(b.tilex as i16).wrapping_abs();
    let dy = (a.tiley as i16).wrapping_sub(b.tiley as i16).wrapping_abs();
    if dx > dy { dx } else { dy }
}

/// `T_Chase`'s attack chance: 300 next to the target (or one tile away and
/// less than a quarter tile into the step), else `(tics << 4) / dist`
/// (unsigned 16-bit division).
fn attack_chance(w: &World, ob: &Obj, dist: i16) -> i16 {
    if dist == 0 || (dist == 1 && ob.distance < 0x4000) {
        300
    } else {
        (w.tics as u16).wrapping_shl(4).checked_div(dist as u16).unwrap_or(0) as i16
    }
}

/// `opposite[d]` (DS:0x0E68). Directions are always 0–8; anything else would
/// read past the table in the original.
fn opposite(d: u16) -> u16 {
    dir::OPPOSITE.get(usize::from(d)).copied().unwrap_or(dir::NODIR)
}

/// Snaps to the centre of the current tile (`T_Chase`).
fn snap_to_tile_centre(o: &mut Obj) {
    o.x = (i32::from(o.tilex) << 16).wrapping_add(0x8000);
    o.y = (i32::from(o.tiley) << 16).wrapping_add(0x8000);
}

/// A tile coordinate as the original's signed `int`.
fn tile_i32(t: u16) -> i32 {
    i32::from(t as i16)
}

/// `QuitCode(n)` (ghidra 1497:11f8): the original exits to DOS. The simulation
/// reports a fault and the caller returns.
fn quit(w: &mut World, code: u16, what: &str) {
    w.emit(SimEvent::Fault(format!("QuitCode {code}: {what}")));
}

// ---------------------------------------------------------------------------
// Shared enemy routines (actors.rs). Allies run the same code as enemies.
// ---------------------------------------------------------------------------

/// Follow mode: `T_Chase` (ghidra 2390:1d2d) as it runs for class 5, whose
/// attack entry is a bare return (ALLIES.md §3.2).
fn t_chase(w: &mut World, id: ObjId) {
    actors::t_chase(w, id);
}

fn try_walk(w: &mut World, id: ObjId) -> bool {
    actors::try_walk(w, id)
}

fn move_obj(w: &mut World, id: ObjId, mv: i32) {
    actors::move_obj(w, id, mv);
}

fn halve_speed(w: &mut World, id: ObjId) {
    actors::halve_speed(w, id);
}

fn double_speed(w: &mut World, id: ObjId) {
    actors::double_speed(w, id);
}

/// `FirstSighting` (ghidra 1e43:1e38): `T_AllyShoot` wakes classes 8–11.
fn first_sighting(w: &mut World, t: ObjId) {
    actors::first_sighting(w, t);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn miss_damage_is_twice_the_object_address() {
        // objlist slot 1 (DS:0xA665) and slot 99 (DS:0xBFA9), ALLIES.md §4.4.
        assert_eq!(objlist_addr(1).wrapping_mul(2) as i16, 19_658);
        assert_eq!(objlist_addr(99).wrapping_mul(2) as i16, 32_594);
    }

    #[test]
    fn line_step_truncates_and_clamps() {
        assert_eq!(line_step(30_080, 29_824), 258);
        assert_eq!(line_step(-3, 2), -384);
        assert_eq!(line_step(i16::MAX, 1), 0x7FFF);
        assert_eq!(line_step(i16::MIN, 1), -0x7FFF);
    }

    #[test]
    fn actorat_word_mapping() {
        assert_eq!(actorat_word(Occ::Empty), 0);
        assert_eq!(actorat_word(Occ::Mark(0x85)), 0x85);
        assert!(actorat_word(Occ::Obj(0)) >= 0x8C);
    }
}
