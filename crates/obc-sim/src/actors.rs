//! Enemies, projectiles and fire: spawning, shared AI, sighting, damage and
//! death, think and action functions.
//!
//! Ports `WOLFSRC/WL_STATE.C` and `WOLFSRC/WL_ACT2.C` with the OBC deltas of
//! docs/re/ACTORS.md (`COUNT.EXE` segments `1e43` and `2390`), plus the fire,
//! grenade and explosion code of segments `2f4d` and `30d6`
//! (docs/re/PLAYER.md §3.10). Addresses are `ghidra SSSS:OOOO` (base 0x1000);
//! DGROUP data is `DS:0xNNNN`.
//!
//! Every function below was delta-checked against a raw 16-bit disassembly of
//! `COUNT.EXE` (and the Ghidra decompilation where it exists), keeping the
//! original control flow and `US_RndT` call order; each `w.rnd()` call site
//! names the `US_RndT` call it stands for. Nothing here is trace-confirmed yet.
//!
//! Original `actorat` values are modelled by [`Occ`]: `Mark(v)` is a wall
//! (`v < 0x80`) or a door (`0x80 | n`); `Obj(i)` is `&objlist[i]`, whose DS
//! address is `0xA623 + 0x42 * i`. Tests such as "`actorat > 0xA623`" therefore
//! mean "an object other than `objlist[0]`".

use obc_core::fixed::fixed_by_frac;

use crate::allies;
use crate::data::{Func, StateId};
use crate::events::SimEvent;
use crate::player;
use crate::world::{
    AMBUSHTILE, AREATILE, DoorAction, MAPSIZE, MINACTORDIST, MINSIGHT, Obj, ObjId, Occ, Static,
    TILEGLOBAL, World, class, dir, fl,
};

/// Module state kept in `World::act`: the original's actor-related globals
/// that no other module owns.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActorsState {
    /// `ingame` (`DS:0x8CBC`): `SpawnFireActor` plays its sound (and draws
    /// `US_RndT`) only while it is set. The level loader should clear it while
    /// `ScanInfoPlane` runs. Default `true`.
    pub ingame: bool,
    /// Kill-percentage timer (`DS:0xCD87`), written by `KillActor`. No reader
    /// was found (UNKNOWN, Q-ACT-4); kept for parity. Cleared per level.
    pub kill_timer: i16,
    /// Bit 0 of `DS:0x46A2` (Sound Blaster Pro mixer detected). `A_IdleSound`
    /// skips one `US_RndT` when set. HYPOTHESIS(Q-ACT-4): hardware dependent.
    pub sb_pro_mixer: bool,
    /// `DS:0x3268 && DS:0x3308` (lighting mode "generators off" and a debug
    /// key flag): enables `A_DebugSparkle`. Default `false`.
    pub debug_sparkle: bool,
    /// Tremor frames (`DS:0x09D8`), set to 10 by the explosion tile effects.
    /// PlayLoop (another module) counts it down.
    pub tremor: i16,
    /// `DS:0xDD87`, set to 12 by `SpawnGrenade`. Meaning UNKNOWN.
    pub grenade_timer: i16,
    /// Damage `T_ShootAlly` applies on a miss. The original uses an
    /// uninitialised stack word; this port reuses the last computed value.
    /// HYPOTHESIS(Q-017).
    pub shoot_ally_damage: i16,
}

impl Default for ActorsState {
    fn default() -> Self {
        Self {
            ingame: true,
            kill_timer: 0,
            sb_pro_mixer: false,
            debug_sparkle: false,
            tremor: 0,
            grenade_timer: 0,
            shoot_ally_damage: 0,
        }
    }
}

/// State records used by this module (DS offsets, docs/re/ACTORS.md §11).
pub mod st {
    use crate::data::StateId;

    pub const GRENADE_FLY: StateId = 0x12EE;
    pub const GRENADE_EXPLODE: StateId = 0x12FE;
    pub const SLIME_FLY: StateId = 0x133E;
    pub const SLIME_SPLASH: StateId = 0x134E;
    pub const FIRE_STAND: StateId = 0x137E;
    pub const FIRE_CHASE: StateId = 0x138E;
    pub const FIRE_ATTACK: StateId = 0x144E;
    pub const FIRE_BURNOUT: StateId = 0x148E;
    pub const RAT_STAND: StateId = 0x14AE;
    pub const RAT_PATH: StateId = 0x14BE;
    pub const RAT_PAIN: StateId = 0x151E;
    pub const RAT_PAIN1: StateId = 0x152E;
    pub const RAT_SHOOT: StateId = 0x153E;
    pub const RAT_CHASE: StateId = 0x156E;
    pub const RAT_DIE: StateId = 0x15CE;
    pub const GRAT_STAND: StateId = 0x160E;
    pub const GRAT_PATH: StateId = 0x161E;
    pub const GRAT_PAIN: StateId = 0x167E;
    pub const GRAT_PAIN1: StateId = 0x168E;
    pub const GRAT_SHOOT: StateId = 0x169E;
    pub const GRAT_CHASE: StateId = 0x16DE;
    pub const GRAT_DIE: StateId = 0x173E;
    pub const SOLDIER_STAND: StateId = 0x17BE;
    pub const SOLDIER_PATH: StateId = 0x17CE;
    pub const SOLDIER_PAIN: StateId = 0x182E;
    pub const SOLDIER_PAIN1: StateId = 0x183E;
    pub const SOLDIER_SHOOT: StateId = 0x184E;
    pub const SOLDIER_ALERT: StateId = 0x187E;
    pub const SOLDIER_CHASE: StateId = 0x188E;
    pub const SOLDIER_DIE: StateId = 0x18EE;
    pub const STOOL_STAND: StateId = 0x193E;
    pub const CROUCH_STAND: StateId = 0x194E;
    pub const TOILET_STAND: StateId = 0x195E;
    pub const PRONE_STAND: StateId = 0x196E;
    pub const STOOL_RISE: StateId = 0x197E;
    pub const CROUCH_RISE: StateId = 0x19CE;
    pub const TOILET_RISE: StateId = 0x1A1E;
    pub const PRONE_RISE: StateId = 0x1A6E;
    pub const SAUCER_PATH: StateId = 0x1ABE;
    pub const SAUCER_CHASE: StateId = 0x1ACE;
    pub const SAUCER_SHOOT: StateId = 0x1B8E;
    pub const SAUCER_DIE: StateId = 0x1BBE;
    pub const BLACK_STAND: StateId = 0x1C3E;
    pub const BLACK_PATH: StateId = 0x1C4E;
    pub const BLACK_PAIN: StateId = 0x1CAE;
    pub const BLACK_SHOOT: StateId = 0x1CCE;
    pub const BLACK_CHASE: StateId = 0x1D0E;
    pub const BLACK_DIE: StateId = 0x1D6E;
    pub const VEST_STAND: StateId = 0x1DEE;
    pub const VEST_CHASE: StateId = 0x1DFE;
    pub const VEST_DIE: StateId = 0x1E5E;
    pub const VEST_SHOOT: StateId = 0x1EBE;
    pub const BOSS_STAND: StateId = 0x1F1E;
    pub const BOSS_CHASE: StateId = 0x1F2E;
    pub const BOSS_DIE: StateId = 0x1F8E;
    pub const BOSS_SHOOT: StateId = 0x200E;
    pub const GIANT_STAND: StateId = 0x205E;
    pub const GIANT_CHASE: StateId = 0x206E;
    pub const GIANT_DIE: StateId = 0x20CE;
    pub const GIANT_SHOOT: StateId = 0x214E;
    pub const GUNNER_STAND: StateId = 0x21AE;
    pub const GUNNER_CHASE: StateId = 0x21BE;
    pub const GUNNER_DIE: StateId = 0x221E;
    pub const GUNNER_SHOOT: StateId = 0x227E;
    pub const FAT_STAND: StateId = 0x229E;
    pub const MANHOLE_STAND: StateId = 0x22AE;
    pub const MANHOLE_RISE: StateId = 0x22BE;
    pub const FAT_CHASE: StateId = 0x230E;
    pub const FAT_DIE: StateId = 0x236E;
    pub const FAT_SHOOT: StateId = 0x23EE;
    pub const SWAMP_SUBMERGED: StateId = 0x244E;
    pub const SWAMP_RISE: StateId = 0x245E;
    pub const SWAMP_SUBMERGE: StateId = 0x24AE;
    /// The six chase states `0x250E..=0x255E` (no damage taken).
    pub const SWAMP_CHASE: [StateId; 6] = [0x250E, 0x251E, 0x252E, 0x253E, 0x254E, 0x255E];
    pub const SWAMP_DIE: StateId = 0x256E;
    pub const SWAMP_SHOOT: StateId = 0x25AE;
    pub const FIREBALL: StateId = 0x260E;
    pub const FLAMES: StateId = 0x263E;
    pub const EXPLOSION: StateId = 0x283E;
    pub const SPARKLE: [StateId; 4] = [0x293E, 0x294E, 0x295E, 0x296E];
    pub const ALLY_PAIN: StateId = 0x2AA6;
    pub const ALLY_PAIN1: StateId = 0x2AB6;
    pub const ALLY_CHASE: StateId = 0x2AE6;
    pub const ALLY_DIE: StateId = 0x2B46;
}

/// Sound tables in DGROUP (AUDIOMUS indices; docs/re/ACTORS.md §9, file read).
mod snd {
    /// `DS:0x0F1E`: pain sounds of classes 8, 9, 12.
    pub const PAIN: [u16; 2] = [76, 77];
    /// `DS:0x0F22`: soldier sight sounds.
    pub const SIGHT_SOLDIER: [u16; 6] = [54, 55, 56, 53, 53, 51];
    /// `DS:0x0F2E`: red-vest sight sounds.
    pub const SIGHT_VEST: [u16; 2] = [52, 61];
    /// `DS:0x2998`: soldier death screams.
    pub const DEATH_SOLDIER: [u16; 4] = [10, 12, 11, 13];
    /// `DS:0x29A0`: ally death screams.
    pub const DEATH_ALLY: [u16; 5] = [22, 23, 24, 25, 26];
    /// `DS:0x29AA`: soldier death screams while the player holds weapon 3.
    pub const DEATH_SOLDIER_BURN: [u16; 4] = [14, 15, 27, 28];
    /// `DS:0x29B2`: swamp creature.
    pub const DEATH_SWAMP: [u16; 2] = [0, 1];
    /// `DS:0x29B6`: rat.
    pub const DEATH_RAT: [u16; 2] = [2, 3];
    /// `DS:0x29BA`: giant rat.
    pub const DEATH_GRAT: [u16; 2] = [4, 5];
    /// `DS:0x29BE`: saucer.
    pub const DEATH_SAUCER: [u16; 2] = [8, 9];
    /// `DS:0x29C2`: red vest.
    pub const DEATH_VEST: [u16; 2] = [18, 19];
    /// `DS:0x29C6`: gunner.
    pub const DEATH_GUNNER: [u16; 2] = [20, 21];
    /// `DS:0x29CA`: soldier idle sounds.
    pub const IDLE_SOLDIER: [u16; 5] = [54, 55, 56, 51, 53];
    /// `DS:0x29D4`: red-vest idle sounds.
    pub const IDLE_VEST: [u16; 2] = [61, 52];
    /// `DS:0x29D8`: rat attack.
    pub const ATTACK_RAT: [u16; 3] = [30, 31, 32];
    /// `DS:0x29DE`: giant-rat attack.
    pub const ATTACK_GRAT: [u16; 2] = [33, 34];
    /// `DS:0x29E2`: disguised-soldier attack.
    pub const ATTACK_DISGUISED: [u16; 2] = [38, 39];
    /// `DS:0x29E6`: fire attack.
    pub const ATTACK_FIRE: [u16; 2] = [71, 73];
    /// `DS:0x29EA`: swamp-creature attack.
    pub const ATTACK_SWAMP: [u16; 2] = [35, 36];
    /// `DS:0x32FE`: fire spawn.
    pub const FIRE_SPAWN: [u16; 3] = [71, 72, 73];
    /// `DS:0x33BA`: explosion.
    pub const EXPLOSION: [u16; 2] = [74, 75];
}

/// `ICONARROWS` (plane-1 path arrows 90..97, `ghidra 2390:2018`).
const ICONARROWS: u16 = 0x5A;
/// `PROJSIZE` in OBC (`ghidra 2390:0006`; Wolf3D 0x2000).
const PROJSIZE: i32 = 0x1000;
/// Kill-percentage thresholds by difficulty (`DS:0x2FD2`, file read).
const KILL_PERCENT: [i16; 5] = [10, 75, 100, 100, 85];
/// Ally hit points by game type (`DS:0x32EC`, file read).
const ALLY_HP: [i16; 8] = [400, 400, 400, 400, 800, 800, 800, 400];
/// Capacity of `statobjlist` in OBC: `DS:0x7E2C` up to `DS:0x86F6`, 10-byte
/// entries (`SpawnStaticAt`'s end test, `ghidra 2281:02a3`). LIKELY.
const MAXSTATS_OBC: usize = 225;

/// Runs an actor state function (think or action).
pub fn call(w: &mut World, id: ObjId, f: Func) {
    match f {
        Func::TStand => t_stand(w, id),
        Func::TPath => t_path(w, id),
        Func::TChase => t_chase(w, id),
        Func::TBossChase => t_boss_chase(w, id),
        Func::TFatChase => t_fat_chase(w, id),
        Func::TSwampChase => t_swamp_chase(w, id),
        Func::TProjectile => t_projectile(w, id),
        Func::TFireBurnout => t_fire_burnout(w, id),
        Func::TBossDead => t_boss_dead(w, id),
        Func::AShoot => a_shoot(w, id),
        Func::TShoot => t_shoot(w, id),
        Func::AIdleSound => a_idle_sound(w, id),
        Func::ADeathScream => a_death_scream(w, id),
        Func::ASlimeThrow => a_slime_throw(w, id),
        Func::AManholeCover => a_manhole_cover(w, id),
        Func::ADebugSparkle => a_debug_sparkle(w, id),
        // `A_Remove` (ghidra 2390:0c4a) and `A_Nop` (2390:2a49); normally handled by `World::call`.
        Func::ARemove => w.obj_mut(id).state = 0,
        Func::ANop => {}
        // Player and ally functions are dispatched by `World::call`.
        Func::TPlayer | Func::TAttack | Func::TAlly | Func::AAlly | Func::Unknown(_) => {}
    }
}

// ---------------------------------------------------------------------------
// Small helpers
// ---------------------------------------------------------------------------

/// The object an `actorat` value points at, if any.
fn occ_obj(o: Occ) -> Option<ObjId> {
    match o {
        Occ::Obj(i) => Some(i),
        _ => None,
    }
}

/// The object an `actorat` value points at when the original's test is
/// `actorat > 0xA623` (excludes `objlist[0]`).
fn occ_obj_after0(o: Occ) -> Option<ObjId> {
    occ_obj(o).filter(|&i| i != 0)
}

/// Signed 16-bit tile coordinate (`int tilex`).
fn ti(v: u16) -> i32 {
    i32::from(v as i16)
}

/// `max(|a.tilex - b.tilex|, |a.tiley - b.tiley|)` in 16-bit arithmetic.
fn tile_dist(a: &Obj, b: &Obj) -> i16 {
    let dx = (a.tilex as i16).wrapping_sub(b.tilex as i16).wrapping_abs();
    let dy = (a.tiley as i16).wrapping_sub(b.tiley as i16).wrapping_abs();
    if dx > dy { dx } else { dy }
}

/// `opposite[d]`, `nodir` for out-of-range values.
fn opposite(d: u16) -> u16 {
    dir::OPPOSITE.get(d as usize).copied().unwrap_or(dir::NODIR)
}

/// `sintable[angle]` (sign-magnitude), 0 outside the table.
fn sin_sm(w: &World, angle: i16) -> i32 {
    w.trig.sintable.get(angle as u16 as usize).copied().unwrap_or(0)
}

/// `costable[angle]` = `sintable[angle + 90]` (sign-magnitude), 0 outside the table.
fn cos_sm(w: &World, angle: i16) -> i32 {
    w.trig.sintable.get(angle as u16 as usize + 90).copied().unwrap_or(0)
}

/// `PlaySoundAt(id, ob->x, ob->y)`.
fn sound_at_obj(w: &mut World, id: ObjId, sound: u16) {
    let (x, y) = (w.obj(id).x, w.obj(id).y);
    w.play_sound_at(sound, x, y);
}

/// `US_RndT() % n` as a table index.
fn rnd_index(w: &mut World, n: i16) -> usize {
    (w.rnd() % n) as usize
}

/// Snaps an actor to the centre of its tile
/// (`ob->x = ((long)ob->tilex << TILESHIFT) + TILEGLOBAL/2`).
fn snap_to_tile(w: &mut World, id: ObjId) {
    let o = w.obj_mut(id);
    o.x = (i32::from(o.tilex) << 16).wrapping_add(TILEGLOBAL / 2);
    o.y = (i32::from(o.tiley) << 16).wrapping_add(TILEGLOBAL / 2);
}

/// The "waiting for a door to open" step shared by every chase loop. Returns
/// false when the actor has to keep waiting.
fn door_wait(w: &mut World, id: ObjId) -> bool {
    let d = w.obj(id).distance;
    if d < 0 {
        // `OpenDoor(-ob->distance - 1)`, then `doorobjlist[-distance-1].action`
        // (16-bit arithmetic on the low word).
        let door = (d as i16).wrapping_neg().wrapping_sub(1) as u16 as usize;
        w.open_door(door);
        if w.doors.get(door).map(|dd| dd.action) != Some(DoorAction::Open) {
            return false;
        }
        w.obj_mut(id).distance = TILEGLOBAL;
    }
    true
}

/// `speed * tics` (`LXMUL@` with `tics` zero-extended).
fn move_amount(w: &World, id: ObjId) -> i32 {
    w.obj(id).speed.wrapping_mul(i32::from(w.tics as u16))
}

// ---------------------------------------------------------------------------
// Spawning
// ---------------------------------------------------------------------------

/// `SpawnStand(which, x, y, dir)` (ghidra 2390:04ca). `dir` is 0..3 (east, north, west, south).
///
/// Wolf3D `SpawnStand` (`WOLFSRC/WL_ACT2.C`). OBC deltas (CONFIRMED,
/// docs/re/ACTORS.md §6.2): own state table, speed 0x300 (rats
/// `US_RndT() + 0x300` and `flags2 |= NOBLOCK`), the ambush block is skipped
/// when `SpawnNewObj` failed, and in scatter mode (difficulty 3) the area is
/// re-derived from the scattered tile.
#[doc(alias = "SpawnStand")]
pub fn spawn_stand(w: &mut World, which: u16, x: u16, y: u16, dir: u16) {
    let state = match which {
        0 => st::RAT_STAND,
        1 => st::SOLDIER_STAND,
        4 => st::GRAT_STAND,
        5 => st::STOOL_STAND,
        6 => st::CROUCH_STAND,
        7 => st::TOILET_STAND,
        8 => st::PRONE_STAND,
        9 => st::BLACK_STAND,
        12 => st::VEST_STAND,
        _ => {
            // QuitNum(1901) in the original.
            w.emit(SimEvent::Fault(format!("SpawnStand: bad type {which} (QuitNum 1901)")));
            return;
        }
    };
    let new = w.spawn_new_obj(x, y, state);

    let map = plane_index(i32::from(x), i32::from(y));
    if plane0_at(w, map) == AMBUSHTILE {
        if World::in_map(i32::from(x), i32::from(y)) {
            w.tilemap[World::idx(i32::from(x), i32::from(y))] = 0;
        }
        // HYPOTHESIS(K-8): the original's `tile` is uninitialised when no
        // neighbour is a floor tile; this port uses area 0.
        let mut tile = AREATILE;
        ambush_area(w, map, &mut tile);
        plane0_set(w, map, tile);
        if let Some(n) = new {
            if w.gamestate.difficulty == 3 {
                let o = *w.obj(n);
                let map2 = plane_index(ti(o.tilex), ti(o.tiley));
                ambush_area(w, map2, &mut tile);
            }
            let o = w.obj_mut(n);
            o.areanumber = (tile as u8).wrapping_sub(AREATILE as u8);
            o.flags |= fl::AMBUSH;
        }
    }

    let Some(n) = new else { return };
    if !w.loadedgame {
        w.gamestate.killtotal = w.gamestate.killtotal.wrapping_add(1);
    }
    if which == 0 {
        // US_RndT (2390:06d4): rat speed.
        let r = w.rnd();
        let o = w.obj_mut(n);
        o.speed = i32::from(r + 0x300);
        o.flags2 |= fl::NOBLOCK;
    } else {
        w.obj_mut(n).speed = 0x300;
    }
    w.obj_mut(n).obclass = which + 3;
    set_hit_points(w, n, which);
    let o = w.obj_mut(n);
    o.dir = dir.wrapping_mul(2);
    o.flags |= fl::SHOOTABLE;
}

/// Linear index into a map plane (`farmapylookup[y] + x`), unchecked.
fn plane_index(x: i32, y: i32) -> isize {
    (y as isize) * MAPSIZE as isize + x as isize
}

/// Plane 0 word at a linear index, 0 outside the plane.
fn plane0_at(w: &World, i: isize) -> u16 {
    usize::try_from(i).ok().and_then(|i| w.plane0.get(i)).copied().unwrap_or(0)
}

fn plane0_set(w: &mut World, i: isize, v: u16) {
    if let Some(p) = usize::try_from(i).ok().and_then(|i| w.plane0.get_mut(i)) {
        *p = v;
    }
}

/// The ambush-tile neighbour scan of `SpawnStand` (+1, −mapwidth, +mapwidth, −1).
fn ambush_area(w: &World, map: isize, tile: &mut u16) {
    for off in [1, -(MAPSIZE as isize), MAPSIZE as isize, -1] {
        let v = plane0_at(w, map + off);
        if v >= AREATILE {
            *tile = v;
        }
    }
}

/// `SpawnPatrol(which, x, y, dir)` (ghidra 2390:0741). `hover` selects the
/// saucer's zero-speed variant (codes 270–273 vs 274–277).
///
/// Wolf3D `SpawnPatrol`. OBC deltas (CONFIRMED, docs/re/ACTORS.md §6.3):
/// speeds (rat `US_RndT()+0x300`, saucer `US_RndT()*2 + DS:0x9299` or 0), and
/// quirk K-3: the patroller is moved one tile along `dir`, then put back on its
/// own tile unless `actorat` there holds a value ≥ 0x8C (an object pointer or
/// a high door number).
#[doc(alias = "SpawnPatrol")]
pub fn spawn_patrol(w: &mut World, which: u16, x: u16, y: u16, dir: u16, hover: bool) {
    let state = match which {
        0 => st::RAT_PATH,
        1 => st::SOLDIER_PATH,
        3 => st::SAUCER_PATH,
        4 => st::GRAT_PATH,
        _ => {
            w.emit(SimEvent::Fault(format!("SpawnPatrol: bad type {which} (QuitNum 1902)")));
            return;
        }
    };
    let Some(n) = w.spawn_new_obj(x, y, state) else { return };
    if !w.loadedgame {
        w.gamestate.killtotal = w.gamestate.killtotal.wrapping_add(1);
    }
    {
        let o = w.obj_mut(n);
        o.obclass = which + 3;
        o.dir = dir.wrapping_mul(2);
        o.distance = TILEGLOBAL; // DS:0x0D1A
        o.flags |= fl::SHOOTABLE;
        o.active = 1;
    }
    // `DS:0x9299`: 0 for hovering saucers, 0x300 for moving ones.
    let saucer_base: i16 = if hover { 0 } else { 0x300 };
    match which {
        0 => {
            // US_RndT (2390:07f4): rat speed.
            let r = w.rnd();
            let o = w.obj_mut(n);
            o.speed = i32::from(r + 0x300);
            o.flags2 |= fl::NOBLOCK;
        }
        3 if saucer_base == 0 => {
            let o = w.obj_mut(n);
            o.speed = 0;
            o.flags2 |= fl::NOBLOCK;
        }
        3 => {
            // US_RndT (2390:0828): moving saucer speed.
            let r = w.rnd();
            let o = w.obj_mut(n);
            o.speed = i32::from((r << 1).wrapping_add(saucer_base));
            o.flags2 |= fl::NOBLOCK;
        }
        _ => w.obj_mut(n).speed = 0x300,
    }
    set_hit_points(w, n, which);

    // Quirk K-3 (2390:0862..08f6).
    let o = *w.obj(n);
    w.set_actorat(ti(o.tilex), ti(o.tiley), Occ::Empty);
    {
        let m = w.obj_mut(n);
        match dir {
            0 => m.tilex = m.tilex.wrapping_add(1),
            1 => m.tiley = m.tiley.wrapping_sub(1),
            2 => m.tilex = m.tilex.wrapping_sub(1),
            3 => m.tiley = m.tiley.wrapping_add(1),
            _ => {}
        }
    }
    let m = *w.obj(n);
    let keep = match w.actorat(ti(m.tilex), ti(m.tiley)) {
        Occ::Obj(_) => true,
        Occ::Mark(v) => v >= 0x8C,
        Occ::Empty => false,
    };
    if !keep {
        let mm = w.obj_mut(n);
        mm.tilex = o.tilex;
        mm.tiley = o.tiley;
    }
    let m = *w.obj(n);
    w.set_actorat(ti(m.tilex), ti(m.tiley), Occ::Obj(n));
}

/// Special spawners for classes 13, 14, 16, 17, 22, 23 (ghidra 2390:0911,
/// 0f20, 0fdc, 0f7e, 103a, 1094). CONFIRMED (docs/re/ACTORS.md §6.4).
///
/// The tictime writes of the gunner, red giant and fat-man spawners store the
/// value already in `COUNT.EXE` (10) and are not reproduced.
#[doc(alias = "SpawnBoss")]
#[doc(alias = "SpawnGunner")]
#[doc(alias = "SpawnFatMan")]
#[doc(alias = "SpawnRedGiant")]
#[doc(alias = "SpawnManholeFat")]
#[doc(alias = "SpawnSwamp")]
pub fn spawn_special(w: &mut World, class: u16, x: u16, y: u16) {
    // (state, speed, which for SetHitPoints, dir)
    let (state, speed, which, d) = match class {
        class::BOSS => (st::BOSS_STAND, 0x300, 10, dir::SOUTH),
        class::GUNNER => (st::GUNNER_STAND, 0x300, 11, dir::NORTH),
        class::RED_GIANT => (st::GIANT_STAND, 0x300, 13, dir::NORTH),
        class::FAT_MAN => (st::FAT_STAND, 0x300, 14, dir::NORTH),
        class::MANHOLE => (st::MANHOLE_STAND, 0x300, 16, dir::NORTH),
        class::SWAMP => (st::SWAMP_SUBMERGED, 0x100, 17, dir::NORTH),
        _ => {
            w.emit(SimEvent::Fault(format!("spawn_special: class {class} has no spawner")));
            return;
        }
    };
    let Some(n) = w.spawn_new_obj(x, y, state) else { return };
    {
        let o = w.obj_mut(n);
        o.speed = speed;
        o.obclass = class;
    }
    set_hit_points(w, n, which);
    let o = w.obj_mut(n);
    o.dir = d;
    match class {
        class::MANHOLE => o.flags |= fl::SHOOTABLE,
        class::SWAMP => {
            o.flags |= fl::SHOOTABLE | fl::AMBUSH;
            o.flags2 |= fl::NOBLOCK;
        }
        _ => o.flags = fl::SHOOTABLE,
    }
    if !w.loadedgame {
        w.gamestate.killtotal = w.gamestate.killtotal.wrapping_add(1);
    }
}

/// `SetHitPoints(which)` (ghidra 2390:28df): `(US_RndT() / div) * mult + base`
/// with `mult = difficulty + 3`, capped at 5000; allies take
/// `DS:0x32EC[difficulty]`.
/// CONFIRMED (docs/re/ACTORS.md §6.5).
#[doc(alias = "SetHitPoints")]
pub fn set_hit_points(w: &mut World, id: ObjId, which: u16) {
    let mult = w.gamestate.difficulty.wrapping_add(3);
    if which == 2 {
        let hp = ALLY_HP.get(w.gamestate.difficulty as u16 as usize).copied().unwrap_or(400);
        w.obj_mut(id).hitpoints = hp;
        return;
    }
    let (div, base): (i16, i16) = match which {
        0 => (4, 1),
        1 => (4, 175),
        3 => (5, 25),
        4 => (4, 10),
        5..=8 => (4, 250),
        9 => (3, 275),
        10 => (1, 4000),
        11 => (2, 2100),
        12 => (1, 3000),
        13 => (2, 1400),
        14 => (2, 2300),
        15 => (1, 1300),
        16 => (2, 2300),
        17 => (4, 250),
        _ => {
            w.emit(SimEvent::Fault(format!("SetHitPoints: bad type {which} (QuitNum 1903)")));
            (4, 25)
        }
    };
    // US_RndT (2390:29c8).
    let r = w.rnd();
    let mut hp = (r / div).wrapping_mul(mult).wrapping_add(base);
    if hp > 5000 {
        hp = 5000;
    }
    w.obj_mut(id).hitpoints = hp;
}

/// `SpawnFireActor(x, y)` (ghidra 30d6:0161). Returns false if refused.
///
/// Refuses at 80 objects; class 21, speed 0x19, `SetHitPoints(15)`, south,
/// flags `NONMARK|NEVERMARK`; plays sound 71/72/73 while `ingame`. Not
/// counted in `killtotal`. CONFIRMED (decomp read).
#[doc(alias = "SpawnFireActor")]
#[doc(alias = "SpawnFire")]
pub fn spawn_fire_actor(w: &mut World, x: u16, y: u16) -> bool {
    if w.objcount > 0x4F {
        return false;
    }
    let Some(n) = w.spawn_new_obj(x, y, st::FIRE_CHASE) else { return false };
    {
        let o = w.obj_mut(n);
        o.speed = 0x19;
        o.obclass = class::FIRE;
    }
    if w.act.ingame {
        // US_RndT (30d6:01b1): spawn sound.
        let i = rnd_index(w, 3);
        sound_at_obj(w, n, snd::FIRE_SPAWN[i]);
    }
    set_hit_points(w, n, 15);
    let o = w.obj_mut(n);
    o.dir = dir::SOUTH;
    o.flags = fl::NONMARK | fl::NEVERMARK;
    true
}

/// Burning flames at an actor (`SpawnFlame(ob)`, ghidra 2390:0bd5): class 18,
/// state `0x263E`, speed 0x14, the player's angle, no target. The new object's
/// flags stay 0 (so `DoActor` marks it in `actorat`). CONFIRMED.
#[doc(alias = "SpawnFlame")]
pub fn spawn_burning_flames(w: &mut World, at: ObjId) {
    let Some(n) = w.get_new_actor() else { return };
    let src = *w.obj(at);
    let angle = w.plr().angle;
    let o = w.obj_mut(n);
    o.ticcount = 1;
    o.x = src.x;
    o.y = src.y;
    o.tilex = (src.x >> 16) as u16;
    o.tiley = (src.y >> 16) as u16;
    o.angle = angle;
    o.state = st::FLAMES;
    o.obclass = class::FLAME;
    o.active = 1;
    o.speed = 0x14;
    o.dir = dir::NODIR;
    o.target = None;
}

/// Booby-trap blast one tile ahead of the player (ghidra 2390:0ae4): class 24,
/// state `0x283E`, flags `NONMARK`, speed 0, at the player's position plus one
/// tile along the player's angle. CONFIRMED.
#[doc(alias = "SpawnExplosionAhead")]
#[doc(alias = "SpawnBoobyTrapBlast")]
pub fn spawn_explosion_ahead(w: &mut World) {
    let Some(n) = w.get_new_actor() else { return };
    let p = *w.plr();
    let dx = fixed_by_frac(TILEGLOBAL, cos_sm(w, p.angle));
    let dy = fixed_by_frac(TILEGLOBAL, sin_sm(w, p.angle)).wrapping_neg();
    let o = w.obj_mut(n);
    o.ticcount = 1;
    o.x = p.x.wrapping_add(dx);
    o.y = p.y.wrapping_add(dy);
    o.tilex = (o.x >> 16) as u16;
    o.tiley = (o.y >> 16) as u16;
    o.angle = p.angle;
    o.state = st::EXPLOSION;
    o.obclass = class::EXPLOSION;
    o.flags = fl::NONMARK;
    o.active = 1;
    o.speed = 0;
    o.dir = dir::NODIR;
    o.target = None;
}

/// Player grenade launcher (`SpawnGrenade`, ghidra 2f4d:0ff3).
///
/// Class 19, state `0x12EE`, speed 0x2000, flags `NONMARK|NEVERMARK`, from the
/// player's position. With a target the angle is aimed by the original's FPU
/// `atan2` ([`calc_angle`]); without one it is the player's angle. Sets
/// `DS:0xDD87 = 12`. CONFIRMED.
#[doc(alias = "SpawnGrenade")]
pub fn spawn_grenade(w: &mut World, target: Option<ObjId>) {
    let p = *w.plr();
    let angle = match target {
        Some(t) => {
            let t = *w.obj(t);
            calc_angle(t.x.wrapping_sub(p.x), p.y.wrapping_sub(t.y))
        }
        None => p.angle,
    };
    let Some(n) = w.get_new_actor() else { return };
    w.act.grenade_timer = 12;
    let network = w.gamestate.difficulty >= 4;
    let o = w.obj_mut(n);
    o.state = st::GRENADE_FLY;
    o.ticcount = 1;
    o.tilex = p.tilex;
    o.tiley = p.tiley;
    o.x = p.x;
    o.y = p.y;
    o.obclass = class::GRENADE;
    o.dir = dir::NODIR;
    o.angle = angle;
    o.speed = 0x2000;
    o.flags = fl::NONMARK | fl::NEVERMARK;
    o.active = 1;
    o.target = target;
    if network {
        o.temp1 = p.temp1;
        o.temp2 = p.temp2;
    }
}

/// Player flame thrower (`SpawnFlame(player, target)`, ghidra 30d6:0493/0547).
///
/// Class 18 fireball, state `0x260E`, speed 0x3000, flags
/// `NONMARK|NEVERMARK`, from the player's position along the player's angle
/// (or aimed at `target` by [`calc_angle`]). CONFIRMED.
#[doc(alias = "SpawnFlame")]
pub fn spawn_player_flame(w: &mut World, target: Option<ObjId>) {
    let ob = *w.plr();
    let angle = match target {
        Some(t) => {
            let t = *w.obj(t);
            calc_angle(t.x.wrapping_sub(ob.x), ob.y.wrapping_sub(t.y))
        }
        None => ob.angle,
    };
    let Some(n) = w.get_new_actor() else { return };
    let o = w.obj_mut(n);
    o.state = st::FIREBALL;
    o.obclass = class::FLAME;
    o.ticcount = 1;
    o.tilex = ob.tilex;
    o.tiley = ob.tiley;
    o.x = ob.x;
    o.y = ob.y;
    o.dir = dir::NODIR;
    o.angle = angle;
    o.speed = 0x3000;
    o.flags = fl::NONMARK | fl::NEVERMARK;
    o.active = 1;
    o.target = target;
}

/// `A_SlimeThrow` (ghidra 2390:1508): the swamp creature becomes shootable and
/// throws a class-20 slime ball (state `0x133E`, speed 0x2000, flags
/// `NONMARK`) at the player. CONFIRMED.
#[doc(alias = "A_SlimeThrow")]
pub fn a_slime_throw(w: &mut World, id: ObjId) {
    w.obj_mut(id).flags |= fl::SHOOTABLE;
    let ob = *w.obj(id);
    let p = *w.plr();
    let angle = calc_angle(p.x.wrapping_sub(ob.x), ob.y.wrapping_sub(p.y));
    let Some(n) = w.get_new_actor() else { return };
    let player = w.player;
    let o = w.obj_mut(n);
    o.state = st::SLIME_FLY;
    o.obclass = class::SLIME;
    o.ticcount = 1;
    o.tilex = ob.tilex;
    o.tiley = ob.tiley;
    o.x = ob.x;
    o.y = ob.y;
    o.dir = dir::NODIR;
    o.angle = angle;
    o.speed = 0x2000;
    o.flags = fl::NONMARK;
    o.active = 1;
    o.target = Some(player);
}

/// `A_DebugSparkle` (ghidra 2390:1274): only while [`ActorsState::debug_sparkle`]
/// is set, spawns a class-25 sparkle aimed at the player (state chosen by
/// `US_RndT() % 4`, speed 0x5000). HYPOTHESIS: a debug feature.
#[doc(alias = "A_DebugSparkle")]
pub fn a_debug_sparkle(w: &mut World, id: ObjId) {
    if !w.act.debug_sparkle {
        return;
    }
    let ob = *w.obj(id);
    let p = *w.plr();
    let angle = calc_angle(p.x.wrapping_sub(ob.x), ob.y.wrapping_sub(p.y));
    let Some(n) = w.get_new_actor() else { return };
    // US_RndT (2390:1342): sparkle kind.
    let state = st::SPARKLE[rnd_index(w, 4)];
    let player = w.player;
    let o = w.obj_mut(n);
    o.state = state;
    o.obclass = class::SPARKLE;
    o.ticcount = 1;
    o.tilex = ob.tilex;
    o.tiley = ob.tiley;
    o.x = ob.x;
    o.y = ob.y;
    o.dir = dir::NODIR;
    o.angle = angle;
    o.speed = 0x5000;
    o.flags = fl::NONMARK;
    o.active = 1;
    o.target = Some(player);
}

// ---------------------------------------------------------------------------
// Shared AI (WL_STATE.C)
// ---------------------------------------------------------------------------

/// Result of a `CHECKSIDE` test.
enum Side {
    Pass,
    Door(u16),
    Block,
}

/// `CHECKDIAG(x, y)` with OBC's fire ignition. Returns true if passable.
#[doc(alias = "CHECKDIAG")]
fn check_diag(w: &mut World, id: ObjId, x: i32, y: i32) -> bool {
    let temp = w.actorat(x, y);
    if temp.is_empty() {
        return true;
    }
    if w.obj(id).obclass == class::FIRE && is_burnable(w, x, y) {
        ignite(w, x, y);
        return false;
    }
    match temp {
        Occ::Obj(o) => w.obj(o).flags & fl::SHOOTABLE == 0,
        _ => false,
    }
}

/// `CHECKSIDE(x, y)` with OBC's fire ignition and wall-bump hook.
#[doc(alias = "CHECKSIDE")]
fn check_side(w: &mut World, id: ObjId, x: i32, y: i32) -> Side {
    let temp = w.actorat(x, y);
    if temp.is_empty() {
        return Side::Pass;
    }
    if w.obj(id).obclass == class::FIRE && is_burnable(w, x, y) {
        ignite(w, x, y);
        return Side::Block;
    }
    match temp {
        Occ::Mark(v) if v < 0x80 => {
            if matches!(w.tile(x, y), 13 | 29 | 49 | 65 | 69) {
                actor_bump_wall(w, id, x, y);
            }
            Side::Block
        }
        Occ::Mark(v) if v < 0x100 => Side::Door(v & 0x3F),
        Occ::Mark(_) => Side::Block,
        Occ::Obj(o) => {
            if w.obj(o).flags & fl::SHOOTABLE != 0 {
                Side::Block
            } else {
                Side::Pass
            }
        }
        Occ::Empty => Side::Pass,
    }
}

/// A cardinal move test: `CHECKDIAG` for class 15 (red vest, OBC's dog/fake
/// rule), otherwise `CHECKSIDE`. Returns true if passable.
fn check_cardinal(w: &mut World, id: ObjId, x: i32, y: i32, doornum: &mut i16) -> bool {
    if w.obj(id).obclass == class::RED_VEST {
        return check_diag(w, id, x, y);
    }
    match check_side(w, id, x, y) {
        Side::Block => false,
        Side::Door(n) => {
            *doornum = n as i16;
            true
        }
        Side::Pass => true,
    }
}

/// `ActorBumpWall(ob, x, y)` (ghidra 1e43:0cca): an actor walked into a wall
/// whose tile is 13, 29, 49, 65 or 69. Except for classes 3, 6, 7 and 21 it
/// calls `FUN_2f4d_08e8(x, y)`, the special-wall/gate list (`DS:0x46E2`).
#[doc(alias = "ActorBumpWall")]
fn actor_bump_wall(w: &mut World, id: ObjId, x: i32, y: i32) {
    if matches!(w.obj(id).obclass, class::RAT | class::SAUCER | class::GIANT_RAT | class::FIRE) {
        return;
    }
    // TODO(Q-ACT-6): call the special-wall handler `FUN_2f4d_08e8(x, y)` once
    // the special-wall list (DS:0x46E2, owned by the map/doors module) exists.
    let _ = (x, y);
}

/// `TryWalk` (ghidra 1e43:01c3).
///
/// Port of `WOLFSRC/WL_STATE.C` `TryWalk`. OBC deltas (CONFIRMED,
/// docs/re/ACTORS.md §7.3): class 15 uses `CHECKDIAG` on cardinal moves;
/// class 21 ignites burnable tiles instead of moving; cardinal wall bumps call
/// `ActorBumpWall`; the area number is stored only if it is in 0..0x16.
#[doc(alias = "TryWalk")]
pub fn try_walk(w: &mut World, id: ObjId) -> bool {
    let mut doornum: i16 = -1;
    let o = *w.obj(id);
    let (x, y) = (ti(o.tilex), ti(o.tiley));
    if o.obclass == class::INERT {
        let (dx, dy): (i16, i16) = match o.dir {
            dir::NORTH => (0, -1),
            dir::NORTHEAST => (1, -1),
            dir::EAST => (1, 0),
            dir::SOUTHEAST => (1, 1),
            dir::SOUTH => (0, 1),
            dir::SOUTHWEST => (-1, 1),
            dir::WEST => (-1, 0),
            dir::NORTHWEST => (-1, -1),
            _ => (0, 0),
        };
        let m = w.obj_mut(id);
        m.tilex = m.tilex.wrapping_add(dx as u16);
        m.tiley = m.tiley.wrapping_add(dy as u16);
    } else {
        let (dx, dy): (i16, i16) = match o.dir {
            dir::NORTH => {
                if !check_cardinal(w, id, x, y - 1, &mut doornum) {
                    return false;
                }
                (0, -1)
            }
            dir::NORTHEAST => {
                if !check_diag(w, id, x + 1, y - 1)
                    || !check_diag(w, id, x + 1, y)
                    || !check_diag(w, id, x, y - 1)
                {
                    return false;
                }
                (1, -1)
            }
            dir::EAST => {
                if !check_cardinal(w, id, x + 1, y, &mut doornum) {
                    return false;
                }
                (1, 0)
            }
            dir::SOUTHEAST => {
                if !check_diag(w, id, x + 1, y + 1)
                    || !check_diag(w, id, x + 1, y)
                    || !check_diag(w, id, x, y + 1)
                {
                    return false;
                }
                (1, 1)
            }
            dir::SOUTH => {
                if !check_cardinal(w, id, x, y + 1, &mut doornum) {
                    return false;
                }
                (0, 1)
            }
            dir::SOUTHWEST => {
                if !check_diag(w, id, x - 1, y + 1)
                    || !check_diag(w, id, x - 1, y)
                    || !check_diag(w, id, x, y + 1)
                {
                    return false;
                }
                (-1, 1)
            }
            dir::WEST => {
                if !check_cardinal(w, id, x - 1, y, &mut doornum) {
                    return false;
                }
                (-1, 0)
            }
            dir::NORTHWEST => {
                if !check_diag(w, id, x - 1, y - 1)
                    || !check_diag(w, id, x - 1, y)
                    || !check_diag(w, id, x, y - 1)
                {
                    return false;
                }
                (-1, -1)
            }
            dir::NODIR => return false,
            d => {
                // QuitNum(302) in the original.
                w.emit(SimEvent::Fault(format!("TryWalk: bad dir {d} (QuitNum 302)")));
                return false;
            }
        };
        let m = w.obj_mut(id);
        m.tilex = m.tilex.wrapping_add(dx as u16);
        m.tiley = m.tiley.wrapping_add(dy as u16);
    }

    if doornum != -1 {
        w.open_door(doornum as usize);
        w.obj_mut(id).distance = -i32::from(doornum) - 1;
        return true;
    }

    let o = *w.obj(id);
    let area = (w.mapspot(ti(o.tilex), ti(o.tiley), 0) as i16).wrapping_sub(AREATILE as i16);
    let m = w.obj_mut(id);
    if (0..0x16).contains(&area) {
        m.areanumber = area as u8;
    }
    m.distance = TILEGLOBAL;
    true
}

/// `SelectDodgeDir` (ghidra 1e43:0d05).
///
/// Port of `WOLFSRC/WL_STATE.C` `SelectDodgeDir`. OBC delta (CONFIRMED,
/// quirk K-1): it acts only when `US_RndT() % 5 == 0`; otherwise `dir` and
/// `distance` are left unchanged.
#[doc(alias = "SelectDodgeDir")]
pub fn select_dodge_dir(w: &mut World, id: ObjId) {
    // US_RndT (1e43:0d10): the 1-in-5 gate.
    if w.rnd() % 5 != 0 {
        return;
    }
    let o = *w.obj(id);
    let turnaround = if o.flags & fl::FIRSTATTACK != 0 {
        w.obj_mut(id).flags &= !fl::FIRSTATTACK;
        dir::NODIR
    } else {
        opposite(o.dir)
    };
    let p = *w.plr();
    let deltax = (p.tilex as i16).wrapping_sub(o.tilex as i16);
    let deltay = (p.tiley as i16).wrapping_sub(o.tiley as i16);
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
    // US_RndT (1e43:0dcf): randomise the dodge.
    if w.rnd() < 128 {
        dirtry.swap(1, 2);
        dirtry.swap(3, 4);
    }
    dirtry[0] = dir::diagonal(dirtry[1], dirtry[2]);
    for d in dirtry {
        if d == dir::NODIR || d == turnaround {
            continue;
        }
        w.obj_mut(id).dir = d;
        if try_walk(w, id) {
            return;
        }
    }
    if turnaround != dir::NODIR {
        w.obj_mut(id).dir = turnaround;
        if try_walk(w, id) {
            return;
        }
    }
    w.obj_mut(id).dir = dir::NODIR;
}

/// `SelectChaseDir` (ghidra 1e43:0e75). No delta from
/// `WOLFSRC/WL_STATE.C` (CONFIRMED), including the north..west sweep.
#[doc(alias = "SelectChaseDir")]
pub fn select_chase_dir(w: &mut World, id: ObjId) {
    let o = *w.obj(id);
    let olddir = o.dir;
    let turnaround = opposite(olddir);
    let p = *w.plr();
    let deltax = (p.tilex as i16).wrapping_sub(o.tilex as i16);
    let deltay = (p.tiley as i16).wrapping_sub(o.tiley as i16);
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
    for d in [d1, d2, olddir] {
        if d != dir::NODIR {
            w.obj_mut(id).dir = d;
            if try_walk(w, id) {
                return;
            }
        }
    }
    // US_RndT (1e43:0f6c): direction of the search.
    let sweep: [u16; 3] = if w.rnd() > 128 {
        [dir::NORTH, dir::NORTHWEST, dir::WEST]
    } else {
        [dir::WEST, dir::NORTHWEST, dir::NORTH]
    };
    for tdir in sweep {
        if tdir != turnaround {
            w.obj_mut(id).dir = tdir;
            if try_walk(w, id) {
                return;
            }
        }
    }
    if turnaround != dir::NODIR {
        w.obj_mut(id).dir = turnaround;
        if try_walk(w, id) {
            return;
        }
    }
    w.obj_mut(id).dir = dir::NODIR;
}

/// `SelectRunDir` (ghidra 1e43:0fdd). No delta from `WOLFSRC/WL_STATE.C`
/// (CONFIRMED).
#[doc(alias = "SelectRunDir")]
pub fn select_run_dir(w: &mut World, id: ObjId) {
    let o = *w.obj(id);
    let p = *w.plr();
    let deltax = (p.tilex as i16).wrapping_sub(o.tilex as i16);
    let deltay = (p.tiley as i16).wrapping_sub(o.tiley as i16);
    let mut d1 = if deltax < 0 { dir::EAST } else { dir::WEST };
    let mut d2 = if deltay < 0 { dir::SOUTH } else { dir::NORTH };
    if deltay.wrapping_abs() > deltax.wrapping_abs() {
        std::mem::swap(&mut d1, &mut d2);
    }
    for d in [d1, d2] {
        w.obj_mut(id).dir = d;
        if try_walk(w, id) {
            return;
        }
    }
    // US_RndT (1e43:1068): direction of the search.
    let sweep: [u16; 3] = if w.rnd() > 128 {
        [dir::NORTH, dir::NORTHWEST, dir::WEST]
    } else {
        [dir::WEST, dir::NORTHWEST, dir::NORTH]
    };
    for tdir in sweep {
        w.obj_mut(id).dir = tdir;
        if try_walk(w, id) {
            return;
        }
    }
    w.obj_mut(id).dir = dir::NODIR;
}

/// `MoveObj(ob, move)` (ghidra 1e43:10b1).
///
/// Port of `WOLFSRC/WL_STATE.C` `MoveObj`. OBC deltas (CONFIRMED,
/// docs/re/ACTORS.md §7.4): no `areabyplayer` test and no ghost damage; the
/// back-up near the player applies everywhere; a moving fire spawns another
/// fire when `US_RndT() == 200`.
#[doc(alias = "MoveObj")]
pub fn move_obj(w: &mut World, id: ObjId, mv: i32) {
    let o = *w.obj(id);
    let (dx, dy) = match o.dir {
        dir::EAST => (mv, 0),
        dir::NORTHEAST => (mv, mv.wrapping_neg()),
        dir::NORTH => (0, mv.wrapping_neg()),
        dir::NORTHWEST => (mv.wrapping_neg(), mv.wrapping_neg()),
        dir::WEST => (mv.wrapping_neg(), 0),
        dir::SOUTHWEST => (mv.wrapping_neg(), mv),
        dir::SOUTH => (0, mv),
        dir::SOUTHEAST => (mv, mv),
        dir::NODIR => return,
        d => {
            w.emit(SimEvent::Fault(format!("MoveObj: bad dir {d} (QuitNum 303)")));
            return;
        }
    };
    {
        let m = w.obj_mut(id);
        m.x = m.x.wrapping_add(dx);
        m.y = m.y.wrapping_add(dy);
    }
    let m = *w.obj(id);
    let p = *w.plr();
    let deltax = m.x.wrapping_sub(p.x);
    if (-MINACTORDIST..=MINACTORDIST).contains(&deltax) {
        let deltay = m.y.wrapping_sub(p.y);
        if (-MINACTORDIST..=MINACTORDIST).contains(&deltay) {
            // Too close to the player: back up and keep `distance`.
            let b = w.obj_mut(id);
            b.x = b.x.wrapping_sub(dx);
            b.y = b.y.wrapping_sub(dy);
            return;
        }
    }
    let b = w.obj_mut(id);
    b.distance = b.distance.wrapping_sub(mv);
    if b.obclass == class::FIRE {
        // US_RndT (1e43:129f): fire spreads.
        if w.rnd() == 200 {
            let b = *w.obj(id);
            spawn_fire_actor(w, b.tilex, b.tiley);
        }
    }
}

/// `HalveSpeed(ob)` (ghidra 2390:2a15): `speed >>= 1` (arithmetic).
#[doc(alias = "HalveSpeed")]
pub fn halve_speed(w: &mut World, id: ObjId) {
    let o = w.obj_mut(id);
    o.speed >>= 1;
}

/// `DoubleSpeed(ob)` (ghidra 2390:2a2f): `speed <<= 1`. With [`halve_speed`]
/// this clears bit 0 of `speed` (quirk K-2).
#[doc(alias = "DoubleSpeed")]
pub fn double_speed(w: &mut World, id: ObjId) {
    let o = w.obj_mut(id);
    o.speed = o.speed.wrapping_mul(2);
}

/// `HalveSpeed`, `SelectDodgeDir`, `DoubleSpeed` as every chase routine calls them.
fn dodge(w: &mut World, id: ObjId) {
    halve_speed(w, id);
    select_dodge_dir(w, id);
    double_speed(w, id);
}

// ---------------------------------------------------------------------------
// Sighting
// ---------------------------------------------------------------------------

/// `CheckLine` (ghidra 1e43:1982): clear line from `id` to the player.
///
/// Port of `WOLFSRC/WL_STATE.C` `CheckLine` (16-bit arithmetic, `plux`/`pluy`).
/// OBC deltas (CONFIRMED, docs/re/ACTORS.md §7.8): cells whose plane-1 value is
/// 105, 106 or 107 are see-through; `deltafrac == 0` quits in the original
/// (`QuitNum(305/306)`), here it fails safe by returning false.
#[doc(alias = "CheckLine")]
pub fn check_line(w: &World, id: ObjId) -> bool {
    let o = w.obj(id);
    let x1 = (o.x >> 8) as i16;
    let y1 = (o.y >> 8) as i16;
    let xt1 = x1 >> 8;
    let yt1 = y1 >> 8;
    let x2 = w.plux;
    let y2 = w.pluy;
    let p = w.plr();
    let mut xt2 = p.tilex as i16;
    let mut yt2 = p.tiley as i16;

    let xdist = xt2.wrapping_sub(xt1).wrapping_abs();
    if xdist > 0 {
        let (partial, xstep): (i16, i16) =
            if xt2 > xt1 { (256 - (x1 & 0xFF), 1) } else { (x1 & 0xFF, -1) };
        let deltafrac = x2.wrapping_sub(x1).wrapping_abs();
        let delta = y2.wrapping_sub(y1);
        if deltafrac == 0 {
            return false; // QuitNum(305)
        }
        let ystep = clamp_step((i32::from(delta) << 8) / i32::from(deltafrac));
        let mut yfrac = y1.wrapping_add(((i32::from(ystep) * i32::from(partial)) >> 8) as i16);
        let mut x = xt1.wrapping_add(xstep);
        xt2 = xt2.wrapping_add(xstep);
        loop {
            let y = yfrac >> 8;
            yfrac = yfrac.wrapping_add(ystep);
            if !line_cell_clear(w, i32::from(x), i32::from(y), yfrac, ystep) {
                return false;
            }
            x = x.wrapping_add(xstep);
            if x == xt2 {
                break;
            }
        }
    }

    let ydist = yt2.wrapping_sub(yt1).wrapping_abs();
    if ydist > 0 {
        let (partial, ystep): (i16, i16) =
            if yt2 > yt1 { (256 - (y1 & 0xFF), 1) } else { (y1 & 0xFF, -1) };
        let deltafrac = y2.wrapping_sub(y1).wrapping_abs();
        let delta = x2.wrapping_sub(x1);
        if deltafrac == 0 {
            return false; // QuitNum(306)
        }
        let xstep = clamp_step((i32::from(delta) << 8) / i32::from(deltafrac));
        let mut xfrac = x1.wrapping_add(((i32::from(xstep) * i32::from(partial)) >> 8) as i16);
        let mut y = yt1.wrapping_add(ystep);
        yt2 = yt2.wrapping_add(ystep);
        loop {
            let x = xfrac >> 8;
            xfrac = xfrac.wrapping_add(xstep);
            if !line_cell_clear(w, i32::from(x), i32::from(y), xfrac, xstep) {
                return false;
            }
            y = y.wrapping_add(ystep);
            if y == yt2 {
                break;
            }
        }
    }
    true
}

/// `ltemp` clamped to ±0x7FFF (`CheckLine`).
fn clamp_step(ltemp: i32) -> i16 {
    ltemp.clamp(-0x7FFF, 0x7FFF) as i16
}

/// One `CheckLine` cell: see-through plane-1 codes, walls, door openings.
/// `frac` is the already advanced fraction, `step` the per-cell step.
fn line_cell_clear(w: &World, x: i32, y: i32, frac: i16, step: i16) -> bool {
    let value = u16::from(w.tile(x, y));
    if matches!(w.mapspot(x, y, 1), 0x69..=0x6B) {
        return true;
    }
    if value == 0 {
        return true;
    }
    if !(0x80..=0x100).contains(&value) {
        return false;
    }
    let door = usize::from(value & 0xFF7F);
    let intercept = frac.wrapping_sub(step / 2) as u16;
    let position = w.doors.get(door).map_or(0, |d| d.position);
    intercept <= position
}

/// `CheckSight` (ghidra 1e43:1d35). No delta from `WOLFSRC/WL_STATE.C`
/// (CONFIRMED): area test, `MINSIGHT` box, facing test, then [`check_line`].
#[doc(alias = "CheckSight")]
pub fn check_sight(w: &World, id: ObjId) -> bool {
    let o = w.obj(id);
    if !w.area_by_player(o.areanumber) {
        return false;
    }
    let p = w.plr();
    let deltax = p.x.wrapping_sub(o.x);
    let deltay = p.y.wrapping_sub(o.y);
    if deltax > -MINSIGHT && deltax < MINSIGHT && deltay > -MINSIGHT && deltay < MINSIGHT {
        return true;
    }
    match o.dir {
        dir::NORTH if deltay > 0 => return false,
        dir::EAST if deltax < 0 => return false,
        dir::SOUTH if deltay < 0 => return false,
        dir::WEST if deltax > 0 => return false,
        _ => {}
    }
    check_line(w, id)
}

/// `SightPlayer` (ghidra 1e43:2313).
///
/// Port of `WOLFSRC/WL_STATE.C` `SightPlayer`. OBC deltas (CONFIRMED,
/// docs/re/ACTORS.md §7.7): network modes never sight; the swamp creature
/// misses with `US_RndT() < 150`; a second countdown `temp3` (network writer
/// only); per-class reaction times. `FL_ATTACKMODE` set on entry quits the
/// original (`QuitNum(308)`); here it is logged and ignored.
#[doc(alias = "SightPlayer")]
pub fn sight_player(w: &mut World, id: ObjId) -> bool {
    if w.gamestate.difficulty >= 4 {
        return false;
    }
    // US_RndT (1e43:232c): the swamp creature's extra miss chance.
    if w.obj(id).obclass == class::SWAMP && w.rnd() < 150 {
        return false;
    }
    if w.obj(id).flags & fl::ATTACKMODE != 0 {
        w.emit(SimEvent::Fault("SightPlayer: already in attack mode (QuitNum 308)".into()));
    }
    let o = *w.obj(id);
    if o.temp2 != 0 {
        let t = o.temp2.wrapping_sub(w.tics);
        w.obj_mut(id).temp2 = t;
        if t > 0 {
            return false;
        }
        w.obj_mut(id).temp2 = 0;
    } else if o.temp3 != 0 {
        let t = o.temp3.wrapping_sub(w.tics);
        w.obj_mut(id).temp3 = t;
        if t > 0 && !check_sight(w, id) {
            return false;
        }
        w.obj_mut(id).temp3 = 0;
    } else {
        if !w.area_by_player(o.areanumber) {
            return false;
        }
        if o.flags & fl::AMBUSH != 0 {
            if !check_sight(w, id) {
                return false;
            }
            w.obj_mut(id).flags &= !fl::AMBUSH;
        } else if !w.madenoise && !check_sight(w, id) {
            return false;
        }
        // Reaction time (table 1e43:246f); US_RndT where a range is used.
        let reaction = match o.obclass {
            class::RAT | class::FIRE | class::MANHOLE | class::SWAMP => Some(w.rnd() / 4 + 1),
            class::SOLDIER | class::ALLY => Some(2),
            6..=17 => Some(w.rnd() / 6 + 1),
            _ => None,
        };
        if let Some(r) = reaction {
            w.obj_mut(id).temp2 = r;
        }
        return false;
    }
    first_sighting(w, id);
    true
}

/// `FirstSighting` (ghidra 1e43:1e38): per-class sound, chase state and speed
/// (table 1e43:22e9), then `distance < 0 → 0`, `ATTACKMODE|FIRSTATTACK`,
/// `active`. CONFIRMED (docs/re/ACTORS.md §7.7).
#[doc(alias = "FirstSighting")]
pub fn first_sighting(w: &mut World, id: ObjId) {
    if w.gamestate.difficulty >= 4 {
        return;
    }
    // (sound, state, speed multiplier; 0 = no change).
    let cls = w.obj(id).obclass;
    let mut set_speed: Option<i32> = None;
    let (sound, state, mult): (Option<u16>, Option<StateId>, i32) = match cls {
        class::RAT => (None, Some(st::RAT_CHASE), 3),
        class::SOLDIER => (Some(soldier_sight(w)), Some(st::SOLDIER_ALERT), 5),
        class::ALLY => (None, Some(st::ALLY_CHASE), 4),
        class::SAUCER => (None, Some(st::SAUCER_CHASE), 5),
        class::GIANT_RAT => (None, Some(st::GRAT_CHASE), 3),
        class::STOOL => (Some(soldier_sight(w)), Some(st::STOOL_RISE), 4),
        class::CROUCH => (Some(soldier_sight(w)), Some(st::CROUCH_RISE), 4),
        class::TOILET => (Some(soldier_sight(w)), Some(st::TOILET_RISE), 4),
        class::PRONE => (Some(soldier_sight(w)), Some(st::PRONE_RISE), 4),
        class::BLACK_SUIT => (Some(soldier_sight(w)), Some(st::BLACK_CHASE), 5),
        class::BOSS => {
            set_speed = Some(0x900);
            (Some(58), Some(st::BOSS_CHASE), 0)
        }
        class::GUNNER => (Some(60), Some(st::GUNNER_CHASE), 3),
        class::RED_VEST => {
            // US_RndT (1e43:2253).
            let i = rnd_index(w, 2);
            (Some(snd::SIGHT_VEST[i]), Some(st::VEST_CHASE), 3)
        }
        class::RED_GIANT => (Some(57), Some(st::GIANT_CHASE), 3),
        class::FAT_MAN => (Some(57), Some(st::FAT_CHASE), 3),
        class::FLAME => (None, Some(st::FIREBALL), 0),
        class::FIRE => (Some(71), Some(st::FIRE_CHASE), 3),
        class::MANHOLE => (Some(57), Some(st::MANHOLE_RISE), 3),
        class::SWAMP => (Some(50), Some(st::SWAMP_RISE), 3),
        _ => (None, None, 0),
    };
    if let Some(s) = sound {
        sound_at_obj(w, id, s);
    }
    if let Some(s) = state {
        w.new_state(id, s);
    }
    let o = w.obj_mut(id);
    if mult != 0 {
        o.speed = o.speed.wrapping_mul(mult);
    }
    if let Some(s) = set_speed {
        o.speed = s;
    }
    if o.distance < 0 {
        o.distance = 0;
    }
    o.flags |= fl::ATTACKMODE | fl::FIRSTATTACK;
    o.active = 1;
    // The trailing `switch (dir)` (debug flag DS:0x0954) only validates `dir`.
}

/// A soldier sight sound (`DS:0x0F22[US_RndT() % 6]`).
fn soldier_sight(w: &mut World) -> u16 {
    // US_RndT (1e43:1efd and the other soldier-type cases).
    snd::SIGHT_SOLDIER[rnd_index(w, 6)]
}

// ---------------------------------------------------------------------------
// Damage and death
// ---------------------------------------------------------------------------

/// `DropItem(item, x, y)` (ghidra 1e43:12e6): Wolf3D's free-spot search
/// (the tile, then the 3×3 block, x outer); nothing in network modes.
#[doc(alias = "DropItem")]
pub fn drop_item(w: &mut World, item: i16, tilex: i32, tiley: i32) {
    if w.gamestate.difficulty >= 4 {
        return;
    }
    if w.actorat(tilex, tiley).is_empty() {
        w.place_item_type(item, tilex as u8, tiley as u8);
        return;
    }
    for x in tilex - 1..=tilex + 1 {
        for y in tiley - 1..=tiley + 1 {
            if w.actorat(x, y).is_empty() {
                w.place_item_type(item, x as u8, y as u8);
                return;
            }
        }
    }
}

/// The BEWITCHED cheat toggle (ghidra 30d6:0456), effective only in cheat mode.
fn toggle_bewitched(w: &mut World) {
    if !w.gamestate.cheatmode {
        return;
    }
    if w.bewitched {
        w.bewitched = false;
        w.emit(SimEvent::Message("BEWITCHED OFF".into()));
    } else {
        w.bewitched = true;
        w.emit(SimEvent::Message("BEWITCHED".into()));
    }
}

/// `KillActor` (ghidra 1e43:1377).
///
/// Port of `WOLFSRC/WL_STATE.C` `KillActor`. OBC deltas (CONFIRMED,
/// docs/re/ACTORS.md §7.5): BEWITCHED is switched off; non-shootable actors
/// are ignored; per-class points, death state and `DropItem`; ally
/// bookkeeping; the kill counter skips allies and ally kills; the
/// kill-percentage timer; `FL_NONMARK` only in single player.
#[doc(alias = "KillActor")]
pub fn kill_actor(w: &mut World, id: ObjId) {
    if w.bewitched {
        toggle_bewitched(w);
    }
    let o = *w.obj(id);
    if o.flags & fl::SHOOTABLE == 0 {
        return;
    }
    // `DS:0x0F1C` (flames on every kill) has no writer: constant false.
    let tilex = (o.x >> 16) as u16;
    let tiley = (o.y >> 16) as u16;
    {
        let m = w.obj_mut(id);
        m.tilex = tilex;
        m.tiley = tiley;
    }
    let (tx, ty) = (ti(tilex), ti(tiley));
    let network = w.gamestate.difficulty >= 4;
    let kill = |w: &mut World, points: i32, state: StateId, drop: Option<i16>| {
        player::give_points(w, points);
        w.new_state(id, state);
        if let Some(item) = drop {
            drop_item(w, item, tx, ty);
        }
    };
    match o.obclass {
        class::RAT => kill(w, 25, st::RAT_DIE, None),
        class::SOLDIER => kill(w, 100, st::SOLDIER_DIE, Some(11)),
        class::ALLY => {
            if network {
                kill(w, 1, st::ALLY_DIE, None);
            } else {
                // DS:0x330A = 1, DS:0x330C = 0: the ally-death portrait animation.
                w.emit(SimEvent::AllyDied);
                w.allycount = i16::from((w.allycount as u8).wrapping_sub(1));
                w.emit(SimEvent::Hud);
                w.new_state(id, st::ALLY_DIE);
                w.obj_mut(id).flags &= !fl::SHOOTABLE;
                drop_item(w, 8, tx, ty);
            }
        }
        class::SAUCER => kill(w, 75, st::SAUCER_DIE, None),
        class::GIANT_RAT => kill(w, 100, st::GRAT_DIE, None),
        class::STOOL..=class::PRONE => kill(w, 100, st::SOLDIER_DIE, Some(11)),
        class::BLACK_SUIT => kill(w, 100, st::BLACK_DIE, Some(5)),
        class::BOSS => kill(w, 1500, st::BOSS_DIE, None),
        class::GUNNER => {
            player::give_points(w, 500);
            set_killxy(w);
            w.new_state(id, st::GUNNER_DIE);
        }
        class::RED_VEST => {
            player::give_points(w, 400);
            set_killxy(w);
            w.new_state(id, st::VEST_DIE);
            drop_item(w, 11, tx, ty);
        }
        class::RED_GIANT => kill(w, 600, st::GIANT_DIE, None),
        class::FAT_MAN | class::MANHOLE => kill(w, 500, st::FAT_DIE, Some(11)),
        class::FIRE => kill(w, 100, st::FIRE_BURNOUT, None),
        class::SWAMP => kill(w, 200, st::SWAMP_DIE, None),
        _ => {}
    }

    if !network {
        if !w.ally.allyfiring && o.obclass != class::ALLY {
            w.gamestate.killcount = w.gamestate.killcount.wrapping_add(1);
            w.emit(SimEvent::Hud);
        }
        // `sprintf(DS:0x874C, "%d", killtotal - killcount)`: status bar text.
        w.emit(SimEvent::Hud);
    }
    // Network: FUN_358f_18e7(ob, x, y) (not ported).

    if !network {
        let gs = w.gamestate;
        if gs.killtotal == 0 {
            // QuitNum(304) in the original (the division would fault).
            w.emit(SimEvent::Fault("KillActor: killtotal is 0 (QuitNum 304)".into()));
        } else {
            let pct = gs.killcount.wrapping_mul(100).wrapping_div(gs.killtotal);
            if w.act.kill_timer <= 0 {
                let threshold =
                    KILL_PERCENT.get(gs.difficulty as u16 as usize).copied().unwrap_or(100);
                if threshold < pct && w.act.kill_timer != -2 {
                    w.act.kill_timer = w.tics << 4;
                }
                if gs.killcount == gs.killtotal {
                    w.act.kill_timer = (w.tics << 4).wrapping_add(1);
                }
            }
        }
    }

    w.obj_mut(id).flags &= !fl::SHOOTABLE;
    w.set_actorat(tx, ty, Occ::Empty);
    if !network {
        w.obj_mut(id).flags |= fl::NONMARK;
    }
    update_kill_ratio(w);
}

/// `gamestate.killx/killy = player->x/y` (classes 14 and 15).
fn set_killxy(w: &mut World) {
    let p = *w.plr();
    w.gamestate.killx = p.x;
    w.gamestate.killy = p.y;
}

/// `UpdateKillRatio` (ghidra 1e43:1710): kill lamp 0/1/2 on the status bar.
#[doc(alias = "UpdateKillRatio")]
fn update_kill_ratio(w: &mut World) {
    w.emit(SimEvent::Hud);
}

/// `DamageActor(ob, damage)` (ghidra 1e43:174d).
///
/// Port of `WOLFSRC/WL_STATE.C` `DamageActor` (`damage` is `unsigned`).
/// OBC deltas (CONFIRMED, docs/re/ACTORS.md §7.6): while
/// `gamestate.chosenweapon == 3`, 1 in 50 kills outright; the swamp creature
/// is immune in its chase states; per-class pain table; no `madenoise`.
#[doc(alias = "DamageActor")]
pub fn damage_actor(w: &mut World, id: ObjId, damage: i16) {
    if w.gamestate.difficulty >= 4 {
        // FUN_358f_135a(ob): network hand-off (not ported).
        return;
    }
    if w.gamestate.chosenweapon == 3 {
        // US_RndT (1e43:1773): instant kill with weapon 3. HYPOTHESIS(Q-ACT-5): flame thrower.
        if w.rnd() % 50 == 0 {
            kill_actor(w, id);
            return;
        }
    }
    let o = *w.obj(id);
    let mut damage = damage as u16;
    if o.flags & fl::ATTACKMODE == 0 {
        damage = damage.wrapping_shl(1);
    }
    if o.obclass == class::SWAMP && st::SWAMP_CHASE.contains(&o.state) {
        return;
    }
    let hp = o.hitpoints.wrapping_sub(damage as i16);
    w.obj_mut(id).hitpoints = hp;
    if hp <= 0 {
        kill_actor(w, id);
        return;
    }
    if o.flags & fl::ATTACKMODE == 0 {
        first_sighting(w, id);
    }
    let odd = w.obj(id).hitpoints & 1 != 0;
    match o.obclass {
        class::RAT => w.new_state(id, if odd { st::RAT_PAIN } else { st::RAT_PAIN1 }),
        class::SOLDIER | class::TOILET | class::PRONE => {
            w.new_state(id, if odd { st::SOLDIER_PAIN } else { st::SOLDIER_PAIN1 })
        }
        class::ALLY => w.new_state(id, if odd { st::ALLY_PAIN } else { st::ALLY_PAIN1 }),
        class::GIANT_RAT => w.new_state(id, if odd { st::GRAT_PAIN } else { st::GRAT_PAIN1 }),
        class::STOOL | class::CROUCH if w.tics & 1 != 0 => {
            // US_RndT (1e43:18b0): pain sound, only on odd `tics`.
            let i = rnd_index(w, 2);
            sound_at_obj(w, id, snd::PAIN[i]);
        }
        class::BLACK_SUIT => {
            w.new_state(id, st::BLACK_PAIN);
            // US_RndT (1e43:1930) and (1e43:194b): 1-in-7 pain sound.
            if w.rnd() % 7 == 0 {
                let i = rnd_index(w, 2);
                sound_at_obj(w, id, snd::PAIN[i]);
            }
        }
        _ => {}
    }
}

// ---------------------------------------------------------------------------
// Think functions (WL_ACT2.C)
// ---------------------------------------------------------------------------

/// `T_Stand` (ghidra 2390:1d1d): `SightPlayer(ob)`. No delta.
#[doc(alias = "T_Stand")]
pub fn t_stand(w: &mut World, id: ObjId) {
    sight_player(w, id);
}

/// `SelectPathDir` (ghidra 2390:1ff4). No delta from `WOLFSRC/WL_ACT2.C`.
#[doc(alias = "SelectPathDir")]
pub fn select_path_dir(w: &mut World, id: ObjId) {
    let o = *w.obj(id);
    let spot = w.mapspot(ti(o.tilex), ti(o.tiley), 1).wrapping_sub(ICONARROWS);
    let m = w.obj_mut(id);
    if spot < 8 {
        m.dir = spot;
    }
    m.distance = TILEGLOBAL;
    if !try_walk(w, id) {
        w.obj_mut(id).dir = dir::NODIR;
    }
}

/// `T_Path` (ghidra 2390:2047).
///
/// Port of `WOLFSRC/WL_ACT2.C` `T_Path`. OBC deltas (CONFIRMED,
/// docs/re/ACTORS.md §7.2): nothing in network modes; the swamp creature acts
/// only when `US_RndT() % 5 == 0`, sets `NOBLOCK` before sighting and on a
/// sighting clears `NOBLOCK` and `FL_SHOOTABLE`. The "hit a wall" `Quit` is
/// logged instead.
#[doc(alias = "T_Path")]
pub fn t_path(w: &mut World, id: ObjId) {
    if w.gamestate.difficulty >= 4 {
        return;
    }
    if w.obj(id).obclass == class::SWAMP {
        // US_RndT (2390:2062).
        if w.rnd() % 5 != 0 {
            return;
        }
        w.obj_mut(id).flags2 |= fl::NOBLOCK;
        if sight_player(w, id) {
            let o = w.obj_mut(id);
            o.flags2 &= !fl::NOBLOCK;
            o.flags &= !fl::SHOOTABLE;
            return;
        }
    } else if sight_player(w, id) {
        return;
    }
    if w.obj(id).dir == dir::NODIR {
        select_path_dir(w, id);
        if w.obj(id).dir == dir::NODIR {
            return;
        }
    }
    let mut mv = move_amount(w, id);
    while mv != 0 {
        if !door_wait(w, id) {
            return;
        }
        let o = *w.obj(id);
        if mv < o.distance {
            move_obj(w, id, mv);
            return;
        }
        if o.tilex > 64 || o.tiley > 64 {
            w.emit(SimEvent::Fault(format!(
                "T_Path hit a wall at {},{}, dir {}",
                o.tilex, o.tiley, o.dir
            )));
            return;
        }
        snap_to_tile(w, id);
        mv = mv.wrapping_sub(o.distance);
        select_path_dir(w, id);
        if w.obj(id).dir == dir::NODIR {
            return;
        }
    }
}

/// `T_Chase` (ghidra 2390:1d2d).
///
/// Port of `WOLFSRC/WL_ACT2.C` `T_Chase`. OBC deltas (CONFIRMED,
/// docs/re/ACTORS.md §7.1): nothing in network modes; attack states by class
/// (table 2390:1fce; classes 5, 14, 17–20, 22+ never attack here; fire adds
/// `FireFlash`); classes 5 and 6 never dodge; dodging halves and doubles the
/// speed around `SelectDodgeDir`. Allies in follow mode call this too.
#[doc(alias = "T_Chase")]
pub fn t_chase(w: &mut World, id: ObjId) {
    if w.gamestate.difficulty >= 4 {
        return;
    }
    let mut dodge_now = false;
    if check_line(w, id) {
        let o = *w.obj(id);
        let p = *w.plr();
        let dist = tile_dist(&o, &p);
        let chance: i16 = if dist == 0 || (dist == 1 && o.distance < 0x4000) {
            300
        } else {
            ((w.tics as u16) << 4).wrapping_div(dist as u16) as i16
        };
        // US_RndT (2390:1db7): attack roll.
        if w.rnd() < chance {
            let attack = match o.obclass {
                class::RAT => Some(st::RAT_SHOOT),
                class::SOLDIER | class::STOOL..=class::PRONE => Some(st::SOLDIER_SHOOT),
                class::SAUCER => Some(st::SAUCER_SHOOT),
                class::GIANT_RAT => Some(st::GRAT_SHOOT),
                class::BLACK_SUIT => Some(st::BLACK_SHOOT),
                class::BOSS => Some(st::BOSS_SHOOT),
                class::RED_VEST => Some(st::VEST_SHOOT),
                class::RED_GIANT => Some(st::GIANT_SHOOT),
                class::FIRE => Some(st::FIRE_ATTACK),
                _ => None,
            };
            if let Some(s) = attack {
                w.new_state(id, s);
                if o.obclass == class::FIRE {
                    fire_flash(w);
                }
            }
            return;
        }
        dodge_now = true;
    }
    let cls = w.obj(id).obclass;
    let chase_only = cls == class::ALLY || cls == class::SAUCER;
    let select = |w: &mut World| {
        if !chase_only && dodge_now {
            dodge(w, id);
        } else {
            select_chase_dir(w, id);
        }
    };
    if w.obj(id).dir == dir::NODIR {
        select(w);
        if w.obj(id).dir == dir::NODIR {
            return;
        }
    }
    let mut mv = move_amount(w, id);
    while mv != 0 {
        if !door_wait(w, id) {
            return;
        }
        let o = *w.obj(id);
        if mv < o.distance {
            move_obj(w, id, mv);
            return;
        }
        snap_to_tile(w, id);
        mv = mv.wrapping_sub(o.distance);
        select(w);
        if w.obj(id).dir == dir::NODIR {
            return;
        }
    }
}

/// The movement part of `T_BossChase`, `T_FatChase` and `T_SwampChase`
/// (Wolf3D `T_Schabb` pattern): run away within 4 tiles, else dodge or chase.
fn boss_chase_move(w: &mut World, id: ObjId, dodge_now: bool, dist: i16) {
    if w.obj(id).dir == dir::NODIR {
        if dodge_now {
            dodge(w, id);
        } else {
            select_chase_dir(w, id);
        }
        if w.obj(id).dir == dir::NODIR {
            return;
        }
    }
    let mut mv = move_amount(w, id);
    while mv != 0 {
        if !door_wait(w, id) {
            return;
        }
        let o = *w.obj(id);
        if mv < o.distance {
            move_obj(w, id, mv);
            return;
        }
        snap_to_tile(w, id);
        mv = mv.wrapping_sub(o.distance);
        if dist < 4 {
            select_run_dir(w, id);
        } else if dodge_now {
            dodge(w, id);
        } else {
            select_chase_dir(w, id);
        }
        if w.obj(id).dir == dir::NODIR {
            return;
        }
    }
}

/// The attack roll of the boss-style chases: `CheckLine` and
/// `US_RndT() < tics << 3` (unsigned compare). `None` when there is no line.
fn boss_attack_roll(w: &mut World, id: ObjId) -> Option<bool> {
    if !check_line(w, id) {
        return None;
    }
    let r = w.rnd() as u16;
    Some(r < (w.tics as u16) << 3)
}

/// `T_BossChase` (ghidra 2390:179b): red vest (class 15) and gunner (14).
/// Attack state `0x227E` for class 14, else `0x1EBE`. CONFIRMED.
#[doc(alias = "T_BossChase")]
pub fn t_boss_chase(w: &mut World, id: ObjId) {
    let dist = tile_dist(w.obj(id), w.plr());
    let mut dodge_now = false;
    // US_RndT (2390:17e8): attack roll.
    match boss_attack_roll(w, id) {
        Some(true) => {
            let s =
                if w.obj(id).obclass == class::GUNNER { st::GUNNER_SHOOT } else { st::VEST_SHOOT };
            w.new_state(id, s);
            return;
        }
        Some(false) => dodge_now = true,
        None => {}
    }
    boss_chase_move(w, id, dodge_now, dist);
}

/// `T_FatChase` (ghidra 2390:1966): the sewer fat man; as [`t_boss_chase`]
/// with attack state `0x23EE`. CONFIRMED.
#[doc(alias = "T_FatChase")]
pub fn t_fat_chase(w: &mut World, id: ObjId) {
    let dist = tile_dist(w.obj(id), w.plr());
    let mut dodge_now = false;
    // US_RndT (2390:19b3): attack roll.
    match boss_attack_roll(w, id) {
        Some(true) => {
            w.new_state(id, st::FAT_SHOOT);
            return;
        }
        Some(false) => dodge_now = true,
        None => {}
    }
    boss_chase_move(w, id, dodge_now, dist);
}

/// `T_SwampChase` (ghidra 2390:1b31): 1 in 20 the creature submerges (flags 0,
/// `NOBLOCK`, speed 0x600); otherwise the boss chase with attack state
/// `0x25AE` (speed set to 0x100). CONFIRMED.
#[doc(alias = "T_SwampChase")]
pub fn t_swamp_chase(w: &mut World, id: ObjId) {
    // US_RndT (2390:1b3c): submerge.
    if w.rnd() % 20 == 0 {
        w.new_state(id, st::SWAMP_SUBMERGE);
        let o = w.obj_mut(id);
        o.flags = 0;
        o.flags2 |= fl::NOBLOCK;
        o.speed = 0x600;
        return;
    }
    let dist = tile_dist(w.obj(id), w.plr());
    let mut dodge_now = false;
    // US_RndT (2390:1baf): attack roll.
    match boss_attack_roll(w, id) {
        Some(true) => {
            w.new_state(id, st::SWAMP_SHOOT);
            w.obj_mut(id).speed = 0x100;
            return;
        }
        Some(false) => dodge_now = true,
        None => {}
    }
    boss_chase_move(w, id, dodge_now, dist);
}

/// `T_FireBurnout` (ghidra 2390:1620): if `|US_RndT() - US_RndT()| < 4` a
/// fire burns out (state `0x148E`) and leaves burning flames. CONFIRMED.
#[doc(alias = "T_FireBurnout")]
pub fn t_fire_burnout(w: &mut World, id: ObjId) {
    // US_RndT (2390:162a) and (2390:1632).
    let a = w.rnd();
    let b = w.rnd();
    if (a - b).abs() < 4 && w.obj(id).obclass == class::FIRE {
        w.new_state(id, st::FIRE_BURNOUT);
        spawn_burning_flames(w, id);
    }
}

/// `T_BossDead` (ghidra 2390:28d1): `playstate = ex_victorious` (6).
#[doc(alias = "T_BossDead")]
pub fn t_boss_dead(w: &mut World, id: ObjId) {
    let _ = id;
    w.playstate = crate::world::ex::VICTORIOUS;
}

// ---------------------------------------------------------------------------
// Actions
// ---------------------------------------------------------------------------

/// `A_Shoot` (ghidra 2390:2321): the class attack sound (table 2390:24dc),
/// then [`t_shoot`]. CONFIRMED.
#[doc(alias = "A_Shoot")]
pub fn a_shoot(w: &mut World, id: ObjId) {
    // US_RndT where the sound is chosen from a table.
    let sound = match w.obj(id).obclass {
        class::RAT => Some(snd::ATTACK_RAT[rnd_index(w, 3)]),
        class::SOLDIER => Some(39),
        class::SAUCER => Some(41),
        class::GIANT_RAT => Some(snd::ATTACK_GRAT[rnd_index(w, 2)]),
        class::STOOL..=class::PRONE => Some(snd::ATTACK_DISGUISED[rnd_index(w, 2)]),
        class::BLACK_SUIT => Some(38),
        class::BOSS => Some(42),
        class::GUNNER => Some(44),
        class::RED_VEST => Some(43),
        class::RED_GIANT => Some(40),
        class::FAT_MAN | class::MANHOLE => Some(37),
        class::FIRE => Some(snd::ATTACK_FIRE[rnd_index(w, 2)]),
        class::SWAMP => Some(snd::ATTACK_SWAMP[rnd_index(w, 2)]),
        _ => None,
    };
    if let Some(s) = sound {
        sound_at_obj(w, id, s);
    }
    t_shoot(w, id);
}

/// `T_Shoot` (ghidra 2390:2506).
///
/// Port of `WOLFSRC/WL_ACT2.C` `T_Shoot`. OBC deltas (CONFIRMED,
/// docs/re/ACTORS.md §7.9): [`t_shoot_ally`] first (single player); no
/// `areabyplayer` test; no firing sound; fire, rats, saucers and giant rats
/// only hurt at close range.
#[doc(alias = "T_Shoot")]
pub fn t_shoot(w: &mut World, id: ObjId) {
    if w.gamestate.difficulty < 4 && t_shoot_ally(w, id) {
        return;
    }
    if !check_line(w, id) {
        return;
    }
    let o = *w.obj(id);
    let p = *w.plr();
    let dist = tile_dist(&o, &p);
    let step = if o.flags & fl::VISABLE != 0 { dist << 4 } else { dist << 3 };
    let hitchance: i16 =
        if w.thrustspeed >= 6000 { 160i16.wrapping_sub(step) } else { 256i16.wrapping_sub(step) };
    // US_RndT (2390:25c5): hit roll.
    if w.rnd() >= hitchance {
        return;
    }
    // US_RndT (2390:25d7 / 25e9 / 25f6): damage.
    let damage = enemy_damage(w, dist);
    let attacker = Some(id);
    match o.obclass {
        class::FIRE => match dist {
            0 => player::take_damage(w, 100, attacker),
            1 => player::take_damage(w, 5, attacker),
            d if d < 0 => player::take_damage(w, 35, attacker),
            _ => {}
        },
        class::RAT => {
            if dist <= 1 {
                player::take_damage(w, 5, attacker);
            }
        }
        class::GIANT_RAT | class::SAUCER => {
            if dist <= 1 {
                player::take_damage(w, damage, attacker);
            }
        }
        _ => player::take_damage(w, damage, attacker),
    }
}

/// Wolf3D's enemy damage by distance: `US_RndT() >> 2/3/4`.
fn enemy_damage(w: &mut World, dist: i16) -> i16 {
    let r = w.rnd();
    if dist < 2 {
        r >> 2
    } else if dist < 4 {
        r >> 3
    } else {
        r >> 4
    }
}

/// `T_ShootAlly` (`EnemyShootAlly`, ghidra 2390:26b9): if the actor
/// `FindNearestVisibleActor` returns is an ally in line of sight, shoot it
/// instead of the player. Quirk K-4: the damage is computed only on a hit
/// (`US_RndT() < 128`) but applied either way. CONFIRMED (code), the miss
/// value is HYPOTHESIS(Q-017).
#[doc(alias = "T_ShootAlly")]
#[doc(alias = "EnemyShootAlly")]
pub fn t_shoot_ally(w: &mut World, id: ObjId) -> bool {
    let Some(t) = allies::enemy_target(w, id) else { return false };
    if w.obj(t).obclass != class::ALLY {
        return false;
    }
    if !allies::can_see(w, t, id) {
        return false;
    }
    let o = *w.obj(id);
    let dist = tile_dist(&o, w.obj(t));
    // US_RndT (2390:2726): hit roll.
    if w.rnd() < 128 {
        w.act.shoot_ally_damage = enemy_damage(w, dist);
    }
    let damage = w.act.shoot_ally_damage;
    match o.obclass {
        class::RAT => {
            if dist <= 1 {
                damage_actor(w, t, 5);
            }
        }
        class::GIANT_RAT => {
            if dist <= 1 {
                damage_actor(w, t, damage);
            }
        }
        class::FIRE => match dist {
            0 => damage_actor(w, t, 75),
            1 => damage_actor(w, t, 5),
            d if d < 0 => damage_actor(w, t, 35),
            _ => {}
        },
        _ => damage_actor(w, t, damage),
    }
    true
}

/// `A_IdleSound` (ghidra 2390:21b7): in the player's area, 1 in 9 and (SB Pro
/// mixer or 1 in 7) the class idle sound; soldiers and black suits need
/// another 1 in 7. CONFIRMED.
#[doc(alias = "A_IdleSound")]
pub fn a_idle_sound(w: &mut World, id: ObjId) {
    let o = *w.obj(id);
    if !w.area_by_player(o.areanumber) {
        return;
    }
    // US_RndT (2390:21d1).
    if w.rnd() % 9 != 0 {
        return;
    }
    // US_RndT (2390:21eb), skipped with an SB Pro mixer (DS:0x46A2 & 1).
    if !w.act.sb_pro_mixer && w.rnd() % 7 != 0 {
        return;
    }
    let sound = match o.obclass {
        class::SOLDIER | class::BLACK_SUIT => {
            // US_RndT (2390:222b) and (2390:2249).
            if w.rnd() % 7 != 0 {
                return;
            }
            snd::IDLE_SOLDIER[rnd_index(w, 5)]
        }
        class::BOSS => 58,
        class::GUNNER => 60,
        // US_RndT (2390:22da).
        class::RED_VEST => snd::IDLE_VEST[rnd_index(w, 2)],
        class::RED_GIANT | class::FAT_MAN => 57,
        class::SWAMP => 50,
        _ => return,
    };
    sound_at_obj(w, id, sound);
}

/// `A_DeathScream` (ghidra 2390:0c59): class death sound (table 2390:0ef6),
/// burning variants while `gamestate.weapon == 3`. The original first blocks
/// for 12 VBLs while digitized sounds 45–49 play (quirk K-9); that wait is
/// presentation only and not simulated. CONFIRMED.
#[doc(alias = "A_DeathScream")]
pub fn a_death_scream(w: &mut World, id: ObjId) {
    let weapon3 = w.gamestate.weapon == 3;
    // US_RndT where the scream is chosen from a table.
    let sound = match w.obj(id).obclass {
        class::RAT => snd::DEATH_RAT[rnd_index(w, 2)],
        class::SOLDIER | class::BLACK_SUIT => {
            if weapon3 {
                snd::DEATH_SOLDIER_BURN[rnd_index(w, 4)]
            } else {
                snd::DEATH_SOLDIER[rnd_index(w, 4)]
            }
        }
        class::ALLY => snd::DEATH_ALLY[rnd_index(w, 5)],
        class::SAUCER => snd::DEATH_SAUCER[rnd_index(w, 2)],
        class::GIANT_RAT => snd::DEATH_GRAT[rnd_index(w, 2)],
        class::STOOL..=class::PRONE => snd::DEATH_SOLDIER[rnd_index(w, 4)],
        class::BOSS => 16,
        class::GUNNER => {
            if weapon3 {
                27
            } else {
                snd::DEATH_GUNNER[rnd_index(w, 2)]
            }
        }
        class::RED_VEST => {
            if weapon3 {
                28
            } else {
                snd::DEATH_VEST[rnd_index(w, 2)]
            }
        }
        class::RED_GIANT => {
            if weapon3 {
                7
            } else {
                6
            }
        }
        class::FAT_MAN | class::MANHOLE => 17,
        class::SWAMP => snd::DEATH_SWAMP[rnd_index(w, 2)],
        _ => return,
    };
    sound_at_obj(w, id, sound);
}

/// `A_ManholeCover` (ghidra 2390:1b17): `SpawnStaticAt(49, tilex, tiley)`, the
/// open manhole. CONFIRMED.
#[doc(alias = "A_ManholeCover")]
pub fn a_manhole_cover(w: &mut World, id: ObjId) {
    let o = *w.obj(id);
    spawn_static_at(w, 49, ti(o.tilex), ti(o.tiley));
}

// ---------------------------------------------------------------------------
// Projectiles
// ---------------------------------------------------------------------------

/// `ProjectileTryMove(ob, &hitx, &hity)` (ghidra 2390:0006): scans the tiles
/// covered by a `PROJSIZE` box (y outer); fails on any `tilemap` value, a
/// wall/door mark or a shootable object. Returns the last tile scanned (as
/// bytes, like the original). The network team test is not ported.
#[doc(alias = "ProjectileTryMove")]
fn projectile_try_move(w: &World, id: ObjId) -> (bool, u8, u8) {
    let o = w.obj(id);
    let xl = o.x.wrapping_sub(PROJSIZE) >> 16;
    let yl = o.y.wrapping_sub(PROJSIZE) >> 16;
    let xh = o.x.wrapping_add(PROJSIZE) >> 16;
    let yh = o.y.wrapping_add(PROJSIZE) >> 16;
    let network = w.gamestate.difficulty >= 4;
    let (mut hx, mut hy) = (0u8, 0u8);
    for y in yl..=yh {
        for x in xl..=xh {
            hx = x as u8;
            hy = y as u8;
            if w.tile(x, y) != 0 {
                return (false, hx, hy);
            }
            match w.actorat(x, y) {
                Occ::Mark(_) => return (false, hx, hy),
                Occ::Obj(i) if i != 0 => {
                    let t = w.obj(i);
                    if t.flags & fl::SHOOTABLE != 0 && (!network || t.temp2 != o.temp2) {
                        return (false, hx, hy);
                    }
                }
                // Network: the player on this tile blocks unless same team (not ported).
                _ => {}
            }
        }
    }
    (true, hx, hy)
}

/// `T_Projectile` (ghidra 2390:0125).
///
/// Port of `WOLFSRC/WL_ACT2.C` `T_Projectile`, rewritten by OBC (CONFIRMED,
/// docs/re/ACTORS.md §10.1): blocked flames ignite, grenades splash and blast
/// (`GrenadeSplash`, explosion tile effects), slime splashes; a projectile
/// within 0xC000 of its `target` hits it (grenades splash first: quirk K-7).
#[doc(alias = "T_Projectile")]
pub fn t_projectile(w: &mut World, id: ObjId) {
    let o = *w.obj(id);
    let mv = o.speed.wrapping_mul(i32::from(w.tics as u16));
    let mut deltax = fixed_by_frac(mv, cos_sm(w, o.angle));
    let mut deltay = fixed_by_frac(mv, sin_sm(w, o.angle)).wrapping_neg();
    if deltax > 0x10000 {
        deltax = 0x10000;
    }
    if deltay > 0x10000 {
        deltay = 0x10000;
    }
    {
        let m = w.obj_mut(id);
        m.x = m.x.wrapping_add(deltax);
        m.y = m.y.wrapping_add(deltay);
    }
    let o = *w.obj(id);
    let (dx, dy) = match o.target {
        Some(t) => {
            let t = w.obj(t);
            (o.x.wrapping_sub(t.x).wrapping_abs(), o.y.wrapping_sub(t.y).wrapping_abs())
        }
        None => (0xC000, 0xC000),
    };

    let (ok, hitx, hity) = projectile_try_move(w, id);
    if !ok {
        match o.obclass {
            class::SLIME => w.obj_mut(id).state = st::SLIME_SPLASH,
            class::GRENADE => {
                let dmg = if w.gamestate.difficulty < 4 {
                    0
                } else {
                    // US_RndT (2390:02cf): network splash damage.
                    (w.rnd() >> 3) + 30
                };
                grenade_splash(w, dmg, id);
                if w.gamestate.difficulty < 4 {
                    explosion_tile_effects(w, hitx, hity, id);
                }
                w.new_state(id, st::GRENADE_EXPLODE);
            }
            class::FLAME => {
                if is_burnable(w, i32::from(hitx), i32::from(hity)) {
                    ignite(w, i32::from(hitx), i32::from(hity));
                    fire_flash(w);
                }
                w.obj_mut(id).state = 0;
                // DebugMessage("F HIT").
            }
            _ => w.obj_mut(id).state = 0,
        }
        return;
    }

    if dx < 0xC000 && dy < 0xC000 {
        let to_player = o.target == Some(w.player);
        let dmg: i16 = match o.obclass {
            class::FLAME => {
                ignite(w, ti(o.tilex), ti(o.tiley));
                fire_flash(w);
                // US_RndT (2390:0453 / 0462).
                if to_player { (w.rnd() >> 1) + 30 } else { w.rnd() + 30 }
            }
            class::GRENADE => {
                if w.gamestate.difficulty < 4 {
                    explosion_tile_effects(w, o.tilex as u8, o.tiley as u8, id);
                }
                // US_RndT (2390:040f / 041e).
                let dmg = if to_player { (w.rnd() >> 1) + 30 } else { w.rnd() + 30 };
                grenade_splash(w, dmg, id);
                dmg
            }
            class::SLIME => {
                // US_RndT (2390:03ab / 03bb), then (2390:03c6) for the screen splat.
                let dmg = if to_player { (w.rnd() >> 3) + 30 } else { w.rnd() + 25 };
                if w.rnd() % 4 != 0 {
                    // DS:0x0D42 = 1; AddScreenSplat(1).
                    w.emit(SimEvent::Splat(1));
                }
                dmg
            }
            class::SPARKLE => {
                w.obj_mut(id).state = 0;
                return;
            }
            // Classes 21-24 and 26 would use an uninitialised value; they have no target.
            _ => 0,
        };
        if to_player {
            player::take_damage(w, dmg, Some(id));
        } else if let Some(t) = o.target {
            damage_actor(w, t, dmg);
        }
        w.obj_mut(id).state = 0;
        return;
    }

    let m = w.obj_mut(id);
    m.tilex = (m.x >> 16) as u16;
    m.tiley = (m.y >> 16) as u16;
}

/// `GrenadeSplash(dmg, ob)` (ghidra 2f4d:116e): every shootable actor whose
/// `actorat` tile is in `[tx-2, tx+2) × [ty-2, ty+2)` around the grenade's
/// position takes `100 - (|Δtilex| + |Δtiley|)` (quirk K-6: asymmetric box,
/// `dmg` ignored); only network games hurt the player (`dmg / 2`). CONFIRMED.
#[doc(alias = "GrenadeSplash")]
pub fn grenade_splash(w: &mut World, dmg: i16, id: ObjId) {
    let o = *w.obj(id);
    let tx = i32::from((o.x >> 16) as i16);
    let ty = i32::from((o.y >> 16) as i16);
    for x in tx - 2..tx + 2 {
        for y in ty - 2..ty + 2 {
            if let Some(i) = occ_obj_after0(w.actorat(x, y)) {
                let t = *w.obj(i);
                if t.flags & fl::SHOOTABLE != 0 {
                    let g = w.obj(id);
                    let d = (t.tilex as i16)
                        .wrapping_sub(g.tilex as i16)
                        .wrapping_abs()
                        .wrapping_add((t.tiley as i16).wrapping_sub(g.tiley as i16).wrapping_abs());
                    damage_actor(w, i, 100i16.wrapping_sub(d));
                }
            }
            let p = *w.plr();
            if w.gamestate.difficulty > 3 && ti(p.tilex) == x && ti(p.tiley) == y {
                player::take_damage(w, dmg >> 1, Some(id));
            }
        }
    }
}

/// Explosion tile effects of a grenade (ghidra 30d6:12b1, *ExplosionTileEffects*).
///
/// CONFIRMED main paths (docs/re/PLAYER.md §3.10): sound 74/75; over the 8×8
/// box `[x-4, x+4)` around the grenade the player takes 300 on the blocking
/// tile or `US_RndT() % 200` elsewhere, and every shootable actor is killed;
/// over `[hit-2, hit+2)` statics become debris and walls are broken, scorched
/// (tile 0x4D) or destroyed; then a screen shake and tremor.
///
/// Differences from PLAYER.md found in the code: door tiles are not
/// excluded (they are scorched like other walls unless their info plane value
/// protects them), and `TakeDamage`'s attacker is the `actorat` value of the
/// previously scanned cell (an uninitialised local for the first cell; this
/// port passes `None` when it is not an object).
#[doc(alias = "ExplosionTileEffects")]
pub fn explosion_tile_effects(w: &mut World, hitx: u8, hity: u8, ob: ObjId) {
    let o = *w.obj(ob);
    let cx = i32::from((o.x >> 16) as i16);
    let cy = i32::from((o.y >> 16) as i16);
    // US_RndT (30d6:12e4): explosion sound (SD_PlaySound, not positional).
    let s = snd::EXPLOSION[rnd_index(w, 2)];
    w.play_sound(s);
    let (hx, hy) = (i32::from(hitx), i32::from(hity));
    let mut last = Occ::Empty;
    for x in cx - 4..cx + 4 {
        for y in cy - 4..cy + 4 {
            let p = *w.plr();
            if ti(p.tilex) == x && ti(p.tiley) == y {
                let attacker = occ_obj(last);
                if hx == x && hy == y {
                    player::take_damage(w, 300, attacker);
                } else {
                    // US_RndT (30d6:133d): self-damage.
                    let r = w.rnd() % 200;
                    player::take_damage(w, r, attacker);
                }
            }
            let a = w.actorat(x, y);
            last = a;
            if let Some(i) = occ_obj_after0(a)
                && w.obj(i).flags & fl::SHOOTABLE != 0
            {
                kill_actor(w, i);
            }
        }
    }

    for x in hx - 2..hx + 2 {
        for y in hy - 2..hy + 2 {
            if x <= 0 || x >= 63 || y <= 0 || y >= 63 {
                continue;
            }
            // Statics on the tile (all slots, removed ones included) become debris.
            let mut i = 0;
            while i < w.statics.len() {
                let s = w.statics[i];
                if i32::from(s.tilex) == x && i32::from(s.tiley) == y {
                    w.statics[i].shape = -1;
                    // US_RndT (30d6:13ea): debris type.
                    let kind = if w.rnd() % 2 != 0 { 0x0C } else { 0x0D };
                    spawn_static_at(w, kind, x, y);
                }
                i += 1;
            }
            let t = w.tile(x, y);
            if t == 0 {
                continue;
            }
            let info = w.mapspot(x, y, 1);
            if info == 0x59 || info == 0x62 {
                continue;
            }
            let t = u16::from(t);
            if t == 0x71 {
                continue;
            }
            let idx = World::idx(x, y);
            if (0x39..=0x3C).contains(&t) {
                w.tilemap[idx] = 0x3C;
                continue;
            }
            if (0x75..=0x78).contains(&t) {
                w.tilemap[idx] = 0x78;
                continue;
            }
            if hx == x && hy == y {
                // The blocking wall. TODO(Q-ACT-6): remove its entry from the
                // special-wall list (DS:0x46E2, count DS:0xE7A8) when that list exists.
                if info == 0x7D {
                    w.tilemap[idx] = 0x4D;
                    w.set_actorat(x, y, Occ::Mark(0x4D));
                    continue;
                }
                w.tilemap[idx] = 0;
                w.set_mapspot(x, y, 1, 0);
                w.set_actorat(x, y, Occ::Empty);
                spawn_static_at(w, 0x0D, x, y);
                // US_RndT (30d6:15d5): 1 in 20 the rubble burns.
                if w.rnd() % 20 == 0 {
                    spawn_fire_actor(w, x as u16, y as u16);
                }
            } else {
                // TODO(Q-ACT-6): walls listed in the special-wall list
                // (DS:0x46E2) are left untouched; that list does not exist yet.
                match info {
                    0x56 | 0x57 | 0x58 | 0x62 => {
                        w.tilemap[idx] = 0;
                        w.set_mapspot(x, y, 1, 0);
                        w.set_actorat(x, y, Occ::Empty);
                        spawn_static_at(w, 0x0D, x, y);
                    }
                    0x7D => {}
                    _ => {
                        w.tilemap[idx] = 0x4D;
                        w.set_mapspot(x, y, 1, 0);
                        w.set_actorat(x, y, Occ::Mark(0x4D));
                    }
                }
            }
        }
    }
    w.emit(SimEvent::Shake(8));
    w.act.tremor = 10;
}

/// `SpawnStaticAt(index, x, y)` (ghidra 2281:02a3): a static from `statinfo`
/// with flags 0 (never blocking, never a bonus), only inside 1..62, reusing
/// the first removed slot whose flags are not 1. CONFIRMED (decomp read).
#[doc(alias = "SpawnStaticAt")]
fn spawn_static_at(w: &mut World, index: usize, x: i32, y: i32) {
    if !(0 < x && x < 63 && 0 < y && y < 63) {
        return;
    }
    let Some(info) = w.data.statinfo.get(index).copied() else { return };
    let s = Static {
        tilex: x as u8,
        tiley: y as u8,
        shape: info.shape,
        flags: 0,
        item: i16::from(info.kind as u8),
    };
    if let Some(slot) = w.statics.iter_mut().find(|s| s.shape == -1 && s.flags != 1) {
        *slot = s;
    } else if w.statics.len() < MAXSTATS_OBC {
        w.statics.push(s);
    }
}

// ---------------------------------------------------------------------------
// Fire
// ---------------------------------------------------------------------------

/// `IsBurnable(x, y)` (ghidra 30d6:0012): the tile is open and holds an
/// object, or the first static listed on it has the bonus flag. CONFIRMED.
#[doc(alias = "IsBurnable")]
pub fn is_burnable(w: &World, x: i32, y: i32) -> bool {
    if w.tile(x, y) != 0 {
        return false;
    }
    if occ_obj(w.actorat(x, y)).is_some() {
        return true;
    }
    w.statics
        .iter()
        .find(|s| i32::from(s.tilex) == x && i32::from(s.tiley) == y)
        .is_some_and(|s| s.flags & fl::BONUS != 0)
}

/// `Ignite`/`BurnTile(x, y)` (ghidra 30d6:0089), single player only.
///
/// A shootable object on the tile: the boss takes `DamageActor(US_RndT())`,
/// the player `TakeDamage(30)`, classes 6, 21 and 23 are immune, anything else
/// gets burning flames and dies. If nothing was hit, every bonus static on the
/// tile becomes a fire actor. CONFIRMED.
#[doc(alias = "Ignite")]
#[doc(alias = "BurnTile")]
pub fn ignite(w: &mut World, x: i32, y: i32) {
    if w.gamestate.difficulty >= 4 {
        return;
    }
    if let Some(i) = occ_obj_after0(w.actorat(x, y))
        && w.obj(i).flags & fl::SHOOTABLE != 0
    {
        match w.obj(i).obclass {
            class::BOSS => {
                // US_RndT (30d6:00ec).
                let r = w.rnd();
                damage_actor(w, i, r);
                return;
            }
            class::PLAYER => {
                player::take_damage(w, 30, Some(i));
                return;
            }
            class::SAUCER | class::FIRE | class::SWAMP => {}
            _ => {
                spawn_burning_flames(w, i);
                kill_actor(w, i);
                return;
            }
        }
    }
    let mut i = 0;
    while i < w.statics.len() {
        let s = w.statics[i];
        if s.shape != -1
            && s.flags & fl::BONUS != 0
            && i32::from(s.tilex) == x
            && i32::from(s.tiley) == y
            && spawn_fire_actor(w, x as u16, y as u16)
        {
            w.statics[i].shape = -1;
        }
        i += 1;
    }
}

/// `FireFlash` (`StartBonusFlash`, ghidra 1942:0f44): single player; 1 in 15
/// a damage flash of `US_RndT() % 100`, otherwise a bonus flash. CONFIRMED.
#[doc(alias = "FireFlash")]
#[doc(alias = "StartBonusFlash")]
pub fn fire_flash(w: &mut World) {
    if w.gamestate.difficulty >= 4 {
        return;
    }
    // US_RndT (1942:0f4e) and (1942:0f5d).
    if w.rnd() % 15 == 0 {
        let strength = w.rnd() % 100;
        w.emit(SimEvent::DamageFlash(strength));
    } else {
        w.emit(SimEvent::BonusFlash);
    }
}

// ---------------------------------------------------------------------------
// Aiming: the original's floating-point atan2
// ---------------------------------------------------------------------------

/// Fixed-point scale of the angle arithmetic: radians × 2^90.
const ANG_FRAC: u32 = 90;
/// Working precision of the constant series: 2^120.
const SERIES_FRAC: u32 = 120;
/// CORDIC iterations (beyond the angle precision).
const CORDIC_STEPS: usize = 96;

/// `atan(1/n)` × 2^120 by its alternating series (integer arithmetic).
const fn atan_inv_series(n: i128) -> i128 {
    let one: i128 = 1 << SERIES_FRAC;
    let n2 = n * n;
    let mut cur = one / n;
    let mut sum: i128 = 0;
    let mut k: i128 = 0;
    while cur != 0 {
        let term = cur / (2 * k + 1);
        if k % 2 == 0 {
            sum += term;
        } else {
            sum -= term;
        }
        cur /= n2;
        k += 1;
    }
    sum
}

/// `atan(2^-i)` × 2^120 for `i >= 1`.
const fn atan_pow2_series(i: u32) -> i128 {
    let mut cur: i128 = 1 << (SERIES_FRAC - i);
    let mut sum: i128 = 0;
    let mut k: i128 = 0;
    while cur != 0 {
        let term = cur / (2 * k + 1);
        if k % 2 == 0 {
            sum += term;
        } else {
            sum -= term;
        }
        cur = if 2 * i >= 127 { 0 } else { cur >> (2 * i) };
        k += 1;
    }
    sum
}

/// Rounds a 2^120 value to the 2^90 angle scale.
const fn to_ang(v: i128) -> i128 {
    (v + (1 << (SERIES_FRAC - ANG_FRAC - 1))) >> (SERIES_FRAC - ANG_FRAC)
}

/// π × 2^90 (Machin's formula).
const PI_FX: i128 = to_ang(4 * (4 * atan_inv_series(5) - atan_inv_series(239)));

/// `atan(2^-i)` × 2^90 for the CORDIC iterations.
const ATAN_TABLE: [i128; CORDIC_STEPS] = {
    let mut t = [0i128; CORDIC_STEPS];
    t[0] = PI_FX / 4;
    let mut i = 1;
    while i < CORDIC_STEPS {
        t[i] = to_ang(atan_pow2_series(i as u32));
        i += 1;
    }
    t
};

/// The double constant `2π` the original adds and divides by
/// (`DS:0x29F4` = 6.283185307179586 = 0x1921FB54442D18 × 2^-50), × 2^90.
const TWO_PI_D_FX: i128 = 0x0019_21FB_5444_2D18 << (ANG_FRAC - 50);

/// `atan2(dy, dx)` × 2^90 in (−π, π], by integer CORDIC (error < 2^-80).
fn atan2_fx(dy: i64, dx: i64) -> i128 {
    if dy == 0 {
        return if dx < 0 { PI_FX } else { 0 };
    }
    if dx == 0 {
        return if dy > 0 { PI_FX / 2 } else { -PI_FX / 2 };
    }
    let (mut x, mut y, mut z) = if dx < 0 {
        (-i128::from(dx), -i128::from(dy), if dy > 0 { PI_FX } else { -PI_FX })
    } else {
        (i128::from(dx), i128::from(dy), 0)
    };
    let mag = x.unsigned_abs().max(y.unsigned_abs());
    let shift = 100u32.saturating_sub(128 - mag.leading_zeros());
    x <<= shift;
    y <<= shift;
    for (i, a) in ATAN_TABLE.iter().enumerate() {
        let (xs, ys) = (x >> i, y >> i);
        if y > 0 {
            x += ys;
            y -= xs;
            z += a;
        } else {
            x -= ys;
            y += xs;
            z -= a;
        }
    }
    z
}

/// Rounds a 2^90-scaled value to the nearest IEEE single (ties to even).
fn round_f32_fx(v: i128) -> i128 {
    if v == 0 {
        return 0;
    }
    let a = v.unsigned_abs();
    let bits = 128 - a.leading_zeros();
    if bits <= 24 {
        return v;
    }
    let sh = bits - 24;
    let mut q = a >> sh;
    let rem = a & ((1u128 << sh) - 1);
    let half = 1u128 << (sh - 1);
    if rem > half || (rem == half && q & 1 == 1) {
        q += 1;
    }
    let r = (q << sh) as i128;
    if v < 0 { -r } else { r }
}

/// The aiming angle of the original's projectile spawners (`A_SlimeThrow`,
/// `SpawnGrenade`, `SpawnFlame`, `A_DebugSparkle`), in degrees 0..=360.
///
/// The original (Wolf3D `T_SchabbThrow` pattern, FPU-emulated) computes
/// `float angle = atan2(dy, dx); if (angle < 0) angle = 2π + angle;
/// iangle = angle / 2π * 360` with `2π` the double at `DS:0x29F4` and 360.0f
/// at `DS:0x29FC`, then truncates (`F_FTOL@`). This port reproduces the two
/// roundings to single precision exactly in integer arithmetic, so exact
/// diagonals give 224 and 314 as in the original. LIKELY: assumes Borland's
/// `atan2` is accurate to well under half a single-precision ulp.
#[doc(alias = "CalcAngle")]
pub fn calc_angle(dx: i32, dy: i32) -> i16 {
    let theta = round_f32_fx(atan2_fx(i64::from(dy), i64::from(dx)));
    let v = if theta < 0 { round_f32_fx(theta + TWO_PI_D_FX) } else { theta };
    ((v * 360) / TWO_PI_D_FX) as i16
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cordic_constants() {
        // π × 2^90 = 4π × 16^22 = 0xC.90FDAA22168C234C4C6628B8… × 16^22, rounded.
        let pi_ref: i128 = 0xC90_FDAA_2216_8C23_4C4C_6629;
        assert!((PI_FX - pi_ref).abs() <= 1, "pi {PI_FX:#x}");
        assert_eq!(ATAN_TABLE[1], to_ang(atan_pow2_series(1)));
        assert_eq!(ATAN_TABLE[90], 1, "atan(2^-90) = 2^-90");
    }

    /// Reference values from Python: `f32(atan2)`, `+2π` rounded to f32,
    /// `int(a / 2π * 360)`.
    #[test]
    fn calc_angle_matches_float_reference() {
        let cases: &[(i32, i32, i16)] = &[
            (1, 0, 0),
            (0, 1, 90),
            (-1, 0, 180),
            (0, -1, 270),
            (1, 1, 45),
            (-1, 1, 135),
            (-1, -1, 224),
            (1, -1, 314),
            (65536, -65536, 314),
            (5, -1, 348),
            (1, -1000000, 270),
            (1000000, -1, 359),
            (-1000000, -1, 180),
            (-1000000, 1, 179),
            (3, 4, 53),
            (-3, 4, 126),
            (1238708, -1663475, 306),
            (2429735, -3384193, 305),
            (-2979025, -2615064, 221),
            (1940937, -3221244, 301),
            (-592267, -3565232, 260),
            (-2752349, 3081063, 131),
            (2821460, -3022325, 313),
            (-156649, -2672393, 266),
            (2927946, -3202595, 312),
            (-2117252, -448976, 191),
            (-3156432, 2460890, 142),
            (-3362334, -485167, 188),
            (-3412777, -1960002, 209),
            (664533, 2837682, 76),
            (-1774106, -2218079, 231),
            (981162, -1162219, 310),
            (-2465317, -1042352, 202),
            (2053490, -2559691, 308),
            (-3140880, -3194363, 225),
            (-738891, 4134149, 100),
            (2979504, 1076210, 19),
            (3617199, 3408868, 43),
            (1872041, 834951, 24),
            (-26398, -1178319, 268),
            (-99045, -2821005, 267),
            (843040, 4112370, 78),
            (1568261, 3335884, 64),
            (636490, -2966198, 282),
            (-2213489, 2820632, 128),
            (-1426700, 1544440, 132),
            (-1644427, 4009135, 112),
            (2880620, -3536516, 309),
            (-2892049, 1069505, 159),
            (1512002, 1680714, 48),
            (4138516, 3459551, 39),
            (-3040654, -2624024, 220),
            (334525, 3759746, 84),
            (-3103786, -3176440, 225),
            (1000045, 3282307, 73),
            (580416, 2278202, 75),
            (35, -6, 350),
            (-48, 9, 169),
            (-5, -29, 260),
            (28, -36, 307),
            (13, -43, 286),
            (-23, 48, 115),
            (-14, -34, 247),
            (44, -19, 336),
            (13, -40, 288),
        ];
        for &(dx, dy, want) in cases {
            assert_eq!(calc_angle(dx, dy), want, "calc_angle({dx}, {dy})");
        }
    }

    #[test]
    fn tiny_negative_angle_wraps_to_360() {
        // f32(θ + 2π) rounds up to f32(2π) > 2π_d, giving 360 as in the original.
        assert_eq!(calc_angle(100_000_000, -1), 360);
    }
}
