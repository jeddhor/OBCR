//! Ally system behaviour on a synthetic world (no game data needed):
//! spawning, the F/A/S keys, Follow movement, Disperse targeting and fire,
//! and the enemy-side helpers. Expected values follow docs/re/ALLIES.md.

use std::collections::BTreeMap;
use std::sync::Arc;

use obc_core::rng::{RNDTABLE, Rng};
use obc_core::trig::Tables;
use obc_sim::allies::{self, FL3_STAY, S_ALLYCHASE1, S_ALLYSHOOT1, S_ALLYSTAY1};
use obc_sim::data::{AttackStep, Func, GameData, State, StateId};
use obc_sim::events::SimEvent;
use obc_sim::world::{AREATILE, MAPSIZE, ObjId, Occ, TicCmd, World, class, dir, fl};

/// The ally state chain (ALLIES.md §2.3): tics, think/action and next only
/// matter here.
fn ally_states() -> BTreeMap<StateId, State> {
    let rec =
        |shape, tics, think, action, next| State { rotate: 1, shape, tics, think, action, next };
    let t = Some(Func::TAlly);
    let mut m = BTreeMap::new();
    m.insert(0x2A26, rec(246, 25, None, None, 0x2A36));
    m.insert(0x2A36, rec(247, 35, t, None, 0x2A26));
    m.insert(0x2AA6, rec(286, 8, None, None, 0x2AB6));
    m.insert(0x2AB6, rec(287, 8, None, None, 0x2AE6));
    m.insert(0x2AC6, rec(288, 6, None, Some(Func::AAlly), 0x2AD6));
    m.insert(0x2AD6, rec(296, 20, None, None, 0x2AE6));
    m.insert(0x2AE6, rec(246, 8, t, None, 0x2AF6));
    m.insert(0x2AF6, rec(246, 8, None, None, 0x2B06));
    m.insert(0x2B06, rec(254, 8, t, None, 0x2B16));
    m.insert(0x2B16, rec(262, 8, None, None, 0x2B26));
    m.insert(0x2B26, rec(262, 8, t, None, 0x2B36));
    m.insert(0x2B36, rec(270, 8, None, None, 0x2AE6));
    m.insert(0x2B46, rec(278, 7, None, None, 0x2BB6));
    m.insert(0x2BB6, rec(285, 0, None, None, 0x2BB6));
    m
}

/// A 64×64 room (walls on the border, area 0 inside) with the player at (32, 32).
fn world() -> World {
    let data = GameData {
        states: ally_states(),
        attackinfo: [[AttackStep::default(); 15]; 5],
        ..GameData::default()
    };
    let mut w = World::new(Arc::new(data), Arc::new(Tables::build()));
    for x in 0..MAPSIZE {
        for y in 0..MAPSIZE {
            let wall = x == 0 || y == 0 || x == MAPSIZE - 1 || y == MAPSIZE - 1;
            if wall {
                w.tilemap[x * MAPSIZE + y] = 1;
                w.actorat[x * MAPSIZE + y] = Occ::Mark(1);
                w.plane0[y * MAPSIZE + x] = 1;
            } else {
                w.plane0[y * MAPSIZE + x] = AREATILE;
            }
        }
    }
    w.areabyplayer[0] = true;
    let p = w.get_new_actor().expect("empty list");
    w.player = p;
    let o = w.obj_mut(p);
    o.obclass = class::PLAYER;
    o.active = 1;
    o.flags = fl::NEVERMARK;
    o.dir = dir::NODIR;
    place(&mut w, p, 32, 32);
    w.gamestate.health = 100;
    w
}

fn place(w: &mut World, id: ObjId, tx: u16, ty: u16) {
    let o = w.obj_mut(id);
    o.tilex = tx;
    o.tiley = ty;
    o.x = (i32::from(tx) << 16) + 0x8000;
    o.y = (i32::from(ty) << 16) + 0x8000;
}

/// Spawns an ally and returns its id (the last object allocated).
fn spawn(w: &mut World, x: u16, y: u16, d: u16) -> ObjId {
    allies::spawn_ally(w, x, y, d);
    w.lastobj.expect("spawned")
}

/// A shootable enemy of `class` at a tile centre.
fn enemy(w: &mut World, cls: u16, x: u16, y: u16) -> ObjId {
    let id = w.get_new_actor().expect("room");
    let o = w.obj_mut(id);
    o.obclass = cls;
    o.active = 1;
    o.flags = fl::SHOOTABLE;
    o.dir = dir::WEST;
    place(w, id, x, y);
    w.set_actorat(i32::from(x), i32::from(y), Occ::Obj(id));
    id
}

fn press(w: &mut World, f: impl FnOnce(&mut TicCmd)) {
    let mut cmd = TicCmd::default();
    f(&mut cmd);
    allies::handle_keys(w, &cmd);
}

fn messages(w: &World) -> Vec<String> {
    w.events
        .iter()
        .filter_map(|e| match e {
            SimEvent::Message(m) => Some(m.clone()),
            _ => None,
        })
        .collect()
}

#[test]
fn spawn_sets_ally_fields() {
    let mut w = world();
    let a = spawn(&mut w, 10, 10, 0);
    let o = *w.obj(a);
    assert_eq!(o.obclass, class::ALLY);
    assert_eq!(o.state, S_ALLYCHASE1);
    assert_eq!(o.dir, dir::EAST);
    assert_eq!(o.speed, 0x400);
    assert_eq!(o.hitpoints, 400);
    assert_eq!(o.distance, 0x10000);
    assert_eq!(o.flags, fl::SHOOTABLE | fl::AMBUSH);
    assert_ne!(o.flags2 & fl::NOBLOCK, 0, "the player walks through allies");
    assert_eq!(o.active, 1);
    // (11,10) is empty: the SpawnPatrol shift is reverted.
    assert_eq!((o.tilex, o.tiley), (10, 10));
    assert_eq!(w.actorat[10 * MAPSIZE + 10], Occ::Obj(a));
    // SpawnNewObj consumed one US_RndT for the tic count.
    assert_eq!(w.rng.index, 1);
}

#[test]
fn spawn_shift_kept_onto_an_actor() {
    let mut w = world();
    let first = spawn(&mut w, 11, 10, 0);
    let second = spawn(&mut w, 10, 10, 0);
    // The next tile holds an actor pointer (>= 0x8C): the shift stays.
    let o = *w.obj(second);
    assert_eq!((o.tilex, o.tiley), (11, 10));
    assert_eq!(o.x >> 16, 10, "x is not moved");
    assert_eq!(w.actorat[11 * MAPSIZE + 10], Occ::Obj(second));
    assert_eq!(w.actorat[10 * MAPSIZE + 10], Occ::Empty);
    assert_eq!(w.obj(first).tilex, 11);
}

#[test]
fn spawn_network_hit_points() {
    let mut w = world();
    w.gamestate.difficulty = 4;
    let a = spawn(&mut w, 10, 10, 3);
    assert_eq!(w.obj(a).hitpoints, 800);
    assert_eq!(w.obj(a).dir, dir::SOUTH);
    assert_eq!(w.obj(a).flags2 & fl::NOBLOCK, 0);
}

#[test]
fn init_allies_counts_and_resets_mode() {
    let mut w = world();
    for x in [10, 12, 14] {
        spawn(&mut w, x, 10, 0);
    }
    let dead = spawn(&mut w, 16, 10, 0);
    w.obj_mut(dead).flags |= fl::NONMARK;
    w.allymode = 1;
    allies::init_allies(&mut w);
    assert_eq!(w.allycount, 3);
    assert_eq!(w.allymode, 0);
    // DrawAllyMode runs before the reset: the HUD keeps the old "F".
    assert_eq!(w.ally.mode_chunk, 0x1D);
}

#[test]
fn f_toggles_mode_for_every_living_ally() {
    let mut w = world();
    let a = spawn(&mut w, 10, 10, 0);
    let b = spawn(&mut w, 12, 10, 0);
    allies::init_allies(&mut w);
    w.obj_mut(a).flags |= fl::ATTACKMODE | fl::VISABLE;
    w.obj_mut(b).state = S_ALLYSHOOT1;
    w.events.clear();

    press(&mut w, |c| c.ally_mode = true);
    assert_eq!(w.allymode, 1);
    assert_eq!(messages(&w), ["FOLLOW PLAYER", "FOLLOW PLAYER"]);
    for id in [a, b] {
        let o = w.obj(id);
        assert_eq!(o.speed, 0x1000);
        assert_eq!(o.state, S_ALLYCHASE1);
        assert_eq!(o.flags, fl::SHOOTABLE | fl::AMBUSH);
    }
    assert_eq!(w.ally.mode_chunk, 0x1D);

    w.events.clear();
    press(&mut w, |c| c.ally_mode = true);
    assert_eq!(w.allymode, 0);
    assert_eq!(messages(&w), ["DISPERSE AND ATTACK", "DISPERSE AND ATTACK"]);
    assert_eq!(w.obj(a).speed, 0xC00);
    assert_eq!(w.ally.mode_chunk, 0x1E);
}

#[test]
fn f_without_allies_flips_silently() {
    let mut w = world();
    allies::init_allies(&mut w);
    w.events.clear();
    press(&mut w, |c| c.ally_mode = true);
    assert_eq!(w.allymode, 1);
    assert!(messages(&w).is_empty());
    assert_eq!(w.ally.mode_chunk, 0x1F, "blank letter with no allies");
}

#[test]
fn follow_walks_toward_the_player() {
    let mut w = world();
    let a = spawn(&mut w, 10, 32, 0);
    allies::init_allies(&mut w);
    press(&mut w, |c| c.ally_mode = true);
    w.tics = 4;
    let start = w.obj(a).x;
    for _ in 0..400 {
        allies::call(&mut w, a, Func::TAlly);
    }
    let o = *w.obj(a);
    let p = *w.plr();
    assert!(o.x > start + (10 << 16), "moved east toward the player");
    assert_eq!(o.tiley, 32);
    // It stops on the centre of the tile next to the player: TryWalk has
    // already claimed the player's tile, but MoveObj backs up every step that
    // would come within MINACTORDIST of the player.
    assert_eq!(o.x, (31 << 16) + 0x8000);
    assert_eq!(p.x - o.x, 0x10000);
    assert_eq!(o.speed, 0x1000);
}

#[test]
fn a_switch_swaps_position_angle_and_health() {
    let mut w = world();
    let a = spawn(&mut w, 10, 10, 0);
    allies::init_allies(&mut w);
    {
        let o = w.obj_mut(a);
        o.hitpoints = 250;
        o.angle = 0;
        o.dir = dir::NORTH;
    }
    w.obj_mut(w.player).angle = 90;
    w.gamestate.health = 80;
    w.events.clear();

    press(&mut w, |c| c.ally_switch = true);
    assert_eq!(messages(&w), ["SWITCH"]);
    assert!(w.events.contains(&SimEvent::Hud));
    let p = *w.plr();
    assert_eq!((p.tilex, p.tiley), (10, 10));
    assert_eq!(p.x, (10 << 16) + 0x8000);
    assert_eq!(p.angle, 0);
    assert_eq!(p.dir, dir::NORTH);
    assert_eq!(w.gamestate.health, 250);
    let o = *w.obj(a);
    assert_eq!((o.tilex, o.tiley), (32, 32));
    assert_eq!(o.hitpoints, 80);
    assert_eq!(o.angle, 90);
    assert_eq!(o.speed, 0x400);
    assert_eq!(o.flags3 & FL3_STAY, 0);
    assert_eq!(w.ally.switchidx, 1);
    assert_eq!(w.ally.switchstay, 0);
}

#[test]
fn switch_caps_health_at_999() {
    let mut w = world();
    let a = spawn(&mut w, 10, 10, 0);
    allies::init_allies(&mut w);
    w.obj_mut(a).hitpoints = 1200;
    press(&mut w, |c| c.ally_switch = true);
    assert_eq!(w.gamestate.health, 999);
}

#[test]
fn a_cycles_through_living_allies() {
    let mut w = world();
    let ids: Vec<_> = [10, 12, 14].into_iter().map(|x| spawn(&mut w, x, 10, 0)).collect();
    allies::init_allies(&mut w);
    let mut expect_tile = Vec::new();
    for _ in 0..4 {
        let before = (w.plr().tilex, w.plr().tiley);
        press(&mut w, |c| c.ally_switch = true);
        expect_tile.push(before);
    }
    // a, b, c, then the index wraps to a (which holds the first position, 32,32).
    assert_eq!((w.plr().tilex, w.plr().tiley), (32, 32));
    assert_eq!((w.obj(ids[0]).tilex, w.obj(ids[0]).tiley), expect_tile[3]);
    assert_eq!((w.obj(ids[1]).tilex, w.obj(ids[1]).tiley), expect_tile[1]);
    assert_eq!((w.obj(ids[2]).tilex, w.obj(ids[2]).tiley), expect_tile[2]);
    assert_eq!(w.ally.switchidx, 1);
}

#[test]
fn s_freezes_the_body_left_behind() {
    let mut w = world();
    let a = spawn(&mut w, 10, 10, 0);
    allies::init_allies(&mut w);
    w.events.clear();
    press(&mut w, |c| c.ally_stay = true);
    assert_eq!(messages(&w), ["STAYPUT & SWITCH"]);
    assert_eq!(w.ally.switchstay, 1);
    let o = *w.obj(a);
    assert_ne!(o.flags3 & FL3_STAY, 0);
    assert_eq!(o.state, S_ALLYSTAY1);

    // T_Ally returns at once: no speed clamp, no movement, no US_RndT.
    w.obj_mut(a).speed = 0x5000;
    let before = *w.obj(a);
    let rng = w.rng;
    for mode in [1, 0] {
        w.allymode = mode;
        allies::call(&mut w, a, Func::TAlly);
    }
    assert_eq!(*w.obj(a), before);
    assert_eq!(w.rng, rng);

    // Only a toggle into Follow clears STAY.
    w.allymode = 1;
    press(&mut w, |c| c.ally_mode = true); // -> Disperse
    assert_ne!(w.obj(a).flags3 & FL3_STAY, 0);
    press(&mut w, |c| c.ally_mode = true); // -> Follow
    assert_eq!(w.obj(a).flags3 & FL3_STAY, 0);
}

#[test]
fn switching_into_a_stay_body_clears_stay() {
    let mut w = world();
    let a = spawn(&mut w, 10, 10, 0);
    let b = spawn(&mut w, 12, 10, 0);
    allies::init_allies(&mut w);
    press(&mut w, |c| c.ally_stay = true); // player <-> a, a frozen at (32,32)
    press(&mut w, |c| c.ally_switch = true); // player <-> b
    press(&mut w, |c| c.ally_switch = true); // index 2 == count: back to a
    assert_eq!((w.plr().tilex, w.plr().tiley), (32, 32));
    assert_eq!(w.obj(a).flags3 & FL3_STAY, 0, "CopyActorState clears STAY");
    assert_eq!(w.obj(b).flags3 & FL3_STAY, 0);
}

#[test]
fn no_allies_message() {
    let mut w = world();
    let dead = spawn(&mut w, 10, 10, 0);
    w.obj_mut(dead).flags = fl::NONMARK; // killed: not shootable, nonmark
    allies::init_allies(&mut w);
    w.events.clear();
    press(&mut w, |c| c.ally_switch = true);
    press(&mut w, |c| c.ally_stay = true);
    assert_eq!(messages(&w), ["NO ALLIES!", "NO ALLIES!"]);
    assert_eq!(w.ally.switchstay, 0);
    assert_eq!((w.plr().tilex, w.plr().tiley), (32, 32));
}

#[test]
fn keys_act_one_per_call_in_order() {
    let mut w = world();
    spawn(&mut w, 10, 10, 0);
    allies::init_allies(&mut w);
    w.events.clear();
    press(&mut w, |c| {
        c.ally_mode = true;
        c.ally_switch = true;
    });
    assert_eq!(messages(&w), ["FOLLOW PLAYER"]);
    press(&mut w, |_| {});
    assert_eq!(messages(&w), ["FOLLOW PLAYER", "SWITCH"]);
    press(&mut w, |_| {});
    assert_eq!(messages(&w).len(), 2);
}

#[test]
fn network_games_ignore_switch_keys() {
    let mut w = world();
    spawn(&mut w, 10, 10, 0);
    w.gamestate.difficulty = 4;
    allies::init_allies(&mut w);
    w.events.clear();
    press(&mut w, |c| {
        c.ally_switch = true;
        c.ally_stay = true;
        c.ally_mode = true;
    });
    press(&mut w, |_| {});
    assert!(messages(&w).is_empty());
    assert_eq!(w.allymode, 0);
}

#[test]
fn disperse_fires_at_the_first_visible_enemy() {
    let mut w = world();
    let a = spawn(&mut w, 10, 10, 0);
    let e = enemy(&mut w, class::SOLDIER, 11, 10);
    allies::init_allies(&mut w);
    // dist 1, distance 0x10000: chance = tics << 4 = 16; RNDTABLE[1] = 8 fires.
    w.rng = Rng::seeded(0);
    allies::call(&mut w, a, Func::TAlly);
    assert_eq!(w.obj(a).state, S_ALLYSHOOT1);
    assert_eq!(w.ally.allytarget, Some(e));
    assert_eq!(w.rng.index, 1);
}

#[test]
fn disperse_without_target_drifts_east() {
    let mut w = world();
    let a = spawn(&mut w, 10, 10, 0);
    allies::init_allies(&mut w);
    w.obj_mut(a).dir = dir::NODIR;
    w.tics = 2;
    allies::call(&mut w, a, Func::TAlly);
    // No target: Δ = (+1, +1), east is tried first.
    assert_eq!(w.obj(a).dir, dir::EAST);
    assert_eq!(w.obj(a).tilex, 11);
    assert!(w.obj(a).x > (10 << 16) + 0x8000);
}

#[test]
fn disperse_through_do_actor_cycles_states() {
    let mut w = world();
    let a = spawn(&mut w, 10, 10, 0);
    allies::init_allies(&mut w);
    let start = w.obj(a).x;
    for _ in 0..70 {
        w.do_actor(a, 1);
    }
    assert_eq!(w.obj(a).obclass, class::ALLY);
    assert!(w.obj(a).x > start, "the chase cycle moves the ally");
    assert!(matches!(w.obj(a).state, 0x2AE6..=0x2B36));
}

#[test]
fn ally_shot_hit_consumes_three_randoms() {
    let mut w = world();
    let a = spawn(&mut w, 10, 10, 0);
    let e = enemy(&mut w, class::SOLDIER, 11, 10);
    w.ally.allytarget = Some(e);
    w.events.clear();
    w.rng = Rng::seeded(0);
    // Sound RNDTABLE[1] % 3 = 2 -> 46; hit roll RNDTABLE[2] = 109 < 128; damage roll.
    allies::call(&mut w, a, Func::AAlly);
    assert_eq!(RNDTABLE[1] % 3, 2);
    let o = *w.obj(a);
    assert!(w.events.contains(&SimEvent::SoundAt { id: 46, x: o.x, y: o.y }));
    assert_eq!(w.rng.index, 3);
    assert_eq!(o.dir, dir::EAST, "opposite of the target's west");
    assert!(!w.ally.allyfiring);
}

#[test]
fn ally_shot_miss_consumes_two_randoms() {
    let mut w = world();
    let a = spawn(&mut w, 10, 10, 0);
    let e = enemy(&mut w, class::SOLDIER, 11, 10);
    // Already alerted, so the real DamageActor adds no FirstSighting draws.
    w.obj_mut(e).flags |= fl::ATTACKMODE;
    w.ally.allytarget = Some(e);
    w.rng = Rng::seeded(2);
    assert!(RNDTABLE[4] >= 128);
    allies::call(&mut w, a, Func::AAlly);
    assert_eq!(w.rng.index, 4);
}

#[test]
fn ally_shot_without_target_still_plays_sound() {
    let mut w = world();
    let a = spawn(&mut w, 10, 10, 0);
    w.events.clear();
    let before = w.rng.index;
    allies::call(&mut w, a, Func::AAlly);
    assert_eq!(w.rng.index, before.wrapping_add(1));
    assert!(w.events.iter().any(|e| matches!(e, SimEvent::SoundAt { id: 45..=47, .. })));
}

#[test]
fn ally_shot_wakes_disguised_soldiers_first() {
    let mut w = world();
    let a = spawn(&mut w, 10, 10, 0);
    let e = enemy(&mut w, class::STOOL, 11, 10);
    w.obj_mut(e).speed = 0x100;
    // Survives the shot, so the woken state is not replaced by a death state.
    w.obj_mut(e).hitpoints = 30000;
    w.ally.allytarget = Some(e);
    allies::call(&mut w, a, Func::AAlly);
    let o = w.obj(e);
    assert_eq!(o.state, 0x197E);
    assert_eq!(o.speed, 0x400);
    assert_eq!(o.flags & (fl::ATTACKMODE | fl::FIRSTATTACK), fl::ATTACKMODE | fl::FIRSTATTACK);
}

#[test]
fn ally_shot_at_an_ally_is_a_fault() {
    let mut w = world();
    let a = spawn(&mut w, 10, 10, 0);
    let b = spawn(&mut w, 12, 10, 0);
    w.ally.allytarget = Some(b);
    w.events.clear();
    allies::call(&mut w, a, Func::AAlly);
    assert!(w.events.iter().any(|e| matches!(e, SimEvent::Fault(m) if m.contains("1602"))));
}

#[test]
fn enemy_target_uses_low_words_only() {
    let mut w = world();
    let e = enemy(&mut w, class::SOLDIER, 10, 10);
    let near = spawn(&mut w, 11, 10, 0);
    let far = spawn(&mut w, 40, 40, 0);
    w.obj_mut(near).x = (11 << 16) | 0x9000;
    for id in [near, far] {
        w.obj_mut(id).flags |= fl::VISABLE;
    }
    // The far ally sits exactly on a tile centre like the enemy: "distance" 0.
    assert_eq!(allies::enemy_target(&mut w, e), Some(far));
    w.obj_mut(far).flags &= !fl::VISABLE;
    assert_eq!(allies::enemy_target(&mut w, e), Some(near));
}

#[test]
fn enemy_target_ignores_non_allies_and_own_class() {
    let mut w = world();
    let e = enemy(&mut w, class::SOLDIER, 10, 10);
    let other = enemy(&mut w, class::SAUCER, 12, 10);
    let same = enemy(&mut w, class::SOLDIER, 13, 10);
    let ally = spawn(&mut w, 30, 30, 0);
    for id in [other, same, ally] {
        w.obj_mut(id).flags |= fl::VISABLE;
    }
    w.obj_mut(ally).x |= 0x7000; // farther in low-word terms than the saucer
    // The nearest candidate is the saucer: not an ally, so the player is shot.
    assert_eq!(allies::enemy_target(&mut w, e), None);
    w.obj_mut(other).flags &= !fl::SHOOTABLE;
    assert_eq!(allies::enemy_target(&mut w, e), Some(ally));
}

#[test]
fn can_see_uses_the_skewed_ray() {
    let mut w = world();
    let a = spawn(&mut w, 10, 10, 0);
    let e = enemy(&mut w, class::SOLDIER, 11, 10);
    assert!(allies::can_see(&mut w, a, e));
    // The line from the enemy toward the ally's low-word position runs through
    // (10, 11), south of the ally: a wall there blocks the view east.
    w.tilemap[10 * MAPSIZE + 11] = 1;
    assert!(!allies::can_see(&mut w, a, e));
    // Plane-1 0x69 makes the tile see-through.
    w.plane1[11 * MAPSIZE + 10] = 0x69;
    assert!(allies::can_see(&mut w, a, e));
}
