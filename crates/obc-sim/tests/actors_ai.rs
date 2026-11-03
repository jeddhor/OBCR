//! Data-gated tests of the enemy, projectile and fire module (docs/re/ACTORS.md).
//!
//! Each test builds an empty 64×64 map (every floor tile in area 0, the
//! player's area) with the state tables read from the user's `COUNT.EXE`.

use std::sync::Arc;

use obc_core::rng::RNDTABLE;
use obc_core::trig::Tables;
use obc_formats::DataDir;
use obc_formats::exe::MzExe;
use obc_sim::actors::{self, st};
use obc_sim::data::{Func, GameData};
use obc_sim::events::SimEvent;
use obc_sim::world::{AREATILE, Door, DoorAction, ObjId, Occ, World, class, dir, ex, fl};

fn world() -> Option<World> {
    let dir = obc_testdata::data_dir()?;
    let exe = MzExe::parse(DataDir::open(dir).ok()?.read("COUNT.EXE").ok()?).ok()?;
    let data = GameData::load(&exe).ok()?;
    let mut w = World::new(Arc::new(data), Arc::new(Tables::build()));
    for v in w.plane0.iter_mut() {
        *v = AREATILE;
    }
    w.areabyplayer[0] = true;
    // InitActorList already gave the player objlist[0].
    let p = w.player;
    let o = w.obj_mut(p);
    o.obclass = class::PLAYER;
    o.flags = fl::NEVERMARK;
    o.active = 1;
    w.gamestate.difficulty = 1;
    w.tics = 1;
    Some(w)
}

macro_rules! world_or_skip {
    () => {
        match world() {
            Some(w) => w,
            None => {
                eprintln!("SKIPPED: original game data not found (set OBC_DATA_DIR)");
                return;
            }
        }
    };
}

/// Puts the player at a tile centre and updates `plux`/`pluy` as `T_Player` does.
fn put_player(w: &mut World, tx: u16, ty: u16, angle: i16) {
    let p = w.player;
    let o = w.obj_mut(p);
    o.tilex = tx;
    o.tiley = ty;
    o.x = (i32::from(tx) << 16) + 0x8000;
    o.y = (i32::from(ty) << 16) + 0x8000;
    o.angle = angle;
    let (x, y) = (o.x, o.y);
    w.plux = (x >> 8) as i16;
    w.pluy = (y >> 8) as i16;
}

fn wall(w: &mut World, x: i32, y: i32) {
    w.tilemap[World::idx(x, y)] = 1;
    w.set_actorat(x, y, Occ::Mark(1));
}

/// The most recently spawned object.
fn last(w: &World) -> ObjId {
    w.lastobj.expect("an object was spawned")
}

/// One PlayLoop actor pass (`for (ob = player; ob; ob = ob->next) DoActor(ob)`).
fn run_actors(w: &mut World) {
    let mut cur = Some(w.player);
    let mut i = 0;
    while let Some(id) = cur {
        w.do_actor(id, i);
        cur = w.obj(id).next;
        i += 1;
        if i > 200 {
            break;
        }
    }
}

#[test]
fn spawn_stand_soldier() {
    let mut w = world_or_skip!();
    actors::spawn_stand(&mut w, 1, 10, 10, 1);
    let id = last(&w);
    let o = *w.obj(id);
    assert_eq!(o.obclass, class::SOLDIER);
    assert_eq!(o.state, st::SOLDIER_STAND);
    assert_eq!(o.dir, dir::NORTH);
    assert_eq!(o.speed, 0x300);
    assert_eq!(o.flags & fl::SHOOTABLE, fl::SHOOTABLE);
    assert_eq!(w.gamestate.killtotal, 1);
    // Stand state has no tictime (no US_RndT in SpawnNewObj); SetHitPoints draws one:
    // (8 / 4) * (difficulty 1 + 3) + 175.
    assert_eq!(w.rng.index, 1);
    assert_eq!(o.hitpoints, i16::from(RNDTABLE[1]) / 4 * 4 + 175);
    assert_eq!(w.actorat(10, 10), Occ::Obj(id));
}

#[test]
fn spawn_stand_rat_speed_then_hitpoints() {
    let mut w = world_or_skip!();
    actors::spawn_stand(&mut w, 0, 5, 5, 0);
    let o = *w.obj(last(&w));
    assert_eq!(o.obclass, class::RAT);
    assert_eq!(o.speed, i32::from(RNDTABLE[1]) + 0x300);
    assert_eq!(o.flags2 & fl::NOBLOCK, fl::NOBLOCK);
    assert_eq!(o.hitpoints, i16::from(RNDTABLE[2]) / 4 * 4 + 1);
    assert_eq!(w.rng.index, 2);
}

#[test]
fn spawn_stand_on_ambush_tile() {
    let mut w = world_or_skip!();
    w.plane0[10 * 64 + 10] = 0x8A;
    w.plane0[10 * 64 + 11] = AREATILE + 3; // east neighbour
    actors::spawn_stand(&mut w, 1, 10, 10, 0);
    let o = *w.obj(last(&w));
    assert_eq!(o.flags & fl::AMBUSH, fl::AMBUSH);
    // The last qualifying neighbour (west, area 0) wins, as in Wolf3D.
    assert_eq!(w.plane0[10 * 64 + 10], AREATILE);
    assert_eq!(o.areanumber, 0);
}

#[test]
fn spawn_patrol_keeps_start_tile() {
    let mut w = world_or_skip!();
    actors::spawn_patrol(&mut w, 1, 20, 20, 0, false);
    let id = last(&w);
    let o = *w.obj(id);
    assert_eq!(o.state, st::SOLDIER_PATH);
    assert_eq!(o.active, 1);
    assert_eq!(o.distance, 0x10000);
    // Quirk K-3: the next tile is empty, so the patroller is put back.
    assert_eq!((o.tilex, o.tiley), (20, 20));
    assert_eq!(w.actorat(20, 20), Occ::Obj(id));
    assert_eq!(w.actorat(21, 20), Occ::Empty);
}

#[test]
fn hovering_saucer_has_no_speed() {
    let mut w = world_or_skip!();
    actors::spawn_patrol(&mut w, 3, 20, 20, 0, true);
    let o = *w.obj(last(&w));
    assert_eq!(o.obclass, class::SAUCER);
    assert_eq!(o.speed, 0);
    assert_eq!(o.flags2 & fl::NOBLOCK, fl::NOBLOCK);
}

#[test]
fn special_spawners() {
    let mut w = world_or_skip!();
    actors::spawn_special(&mut w, class::BOSS, 30, 30);
    let boss = *w.obj(last(&w));
    assert_eq!((boss.obclass, boss.dir, boss.flags), (class::BOSS, dir::SOUTH, fl::SHOOTABLE));
    assert!((4000..=5000).contains(&boss.hitpoints));
    actors::spawn_special(&mut w, class::SWAMP, 32, 30);
    let swamp = *w.obj(last(&w));
    assert_eq!(swamp.speed, 0x100);
    assert_eq!(swamp.flags, fl::SHOOTABLE | fl::AMBUSH);
    assert_eq!(w.gamestate.killtotal, 2);
    assert!(actors::spawn_fire_actor(&mut w, 34, 30));
    let fire = *w.obj(last(&w));
    assert_eq!(fire.obclass, class::FIRE);
    assert_eq!(fire.flags, fl::NONMARK | fl::NEVERMARK);
    assert_eq!(w.gamestate.killtotal, 2, "fire is not counted");
    assert!(w.events.iter().any(|e| matches!(e, SimEvent::SoundAt { id: 71..=73, .. })));
}

#[test]
fn soldier_sights_player_after_reaction_time() {
    let mut w = world_or_skip!();
    put_player(&mut w, 10, 5, 270);
    actors::spawn_stand(&mut w, 1, 10, 10, 1); // facing north
    let id = last(&w);
    assert!(actors::check_line(&w, id));
    assert!(actors::check_sight(&w, id));

    actors::call(&mut w, id, Func::TStand);
    assert_eq!(w.obj(id).temp2, 2, "soldier reaction time");
    assert_eq!(w.obj(id).state, st::SOLDIER_STAND);
    actors::call(&mut w, id, Func::TStand);
    assert_eq!(w.obj(id).temp2, 1);
    let before = w.rng.index;
    actors::call(&mut w, id, Func::TStand);
    let o = *w.obj(id);
    assert_eq!(o.state, st::SOLDIER_ALERT);
    assert_eq!(o.speed, 0x300 * 5);
    assert_eq!(o.flags & (fl::ATTACKMODE | fl::FIRSTATTACK), fl::ATTACKMODE | fl::FIRSTATTACK);
    assert_eq!(w.rng.index, before + 1, "one US_RndT for the sight sound");
    let sound = [54, 55, 56, 53, 53, 51][usize::from(RNDTABLE[usize::from(before) + 1]) % 6];
    assert!(w.events.contains(&SimEvent::SoundAt { id: sound, x: o.x, y: o.y }));
}

#[test]
fn facing_away_needs_noise() {
    let mut w = world_or_skip!();
    put_player(&mut w, 10, 5, 270);
    actors::spawn_stand(&mut w, 1, 10, 10, 3); // facing south, player is north
    let id = last(&w);
    assert!(!actors::check_sight(&w, id));
    actors::call(&mut w, id, Func::TStand);
    assert_eq!(w.obj(id).temp2, 0);
    w.madenoise = true;
    actors::call(&mut w, id, Func::TStand);
    assert_eq!(w.obj(id).temp2, 2);
}

#[test]
fn check_line_walls_and_see_through_codes() {
    let mut w = world_or_skip!();
    put_player(&mut w, 10, 5, 270);
    actors::spawn_stand(&mut w, 1, 10, 10, 1);
    let id = last(&w);
    wall(&mut w, 10, 7);
    assert!(!actors::check_line(&w, id));
    // Plane-1 codes 105..107 are see-through (docs/re/ACTORS.md §7.8).
    w.plane1[7 * 64 + 10] = 105;
    assert!(actors::check_line(&w, id));
    w.plane1[7 * 64 + 10] = 104;
    assert!(!actors::check_line(&w, id));
}

#[test]
fn check_line_through_doors() {
    let mut w = world_or_skip!();
    put_player(&mut w, 14, 10, 0);
    actors::spawn_stand(&mut w, 1, 10, 10, 0);
    let id = last(&w);
    w.doors.push(Door { tilex: 12, tiley: 10, vertical: true, ..Door::default() });
    w.tilemap[World::idx(12, 10)] = 0x80;
    w.doors[0].position = 0;
    assert!(!actors::check_line(&w, id), "closed door blocks");
    w.doors[0].position = 0xFFFF;
    assert!(actors::check_line(&w, id), "open door lets the line pass");
}

#[test]
fn select_dodge_dir_one_in_five_gate() {
    let mut w = world_or_skip!();
    put_player(&mut w, 10, 5, 0);
    actors::spawn_stand(&mut w, 1, 10, 10, 1);
    let id = last(&w);
    w.rng.index = 0; // next US_RndT = 8: 8 % 5 != 0
    actors::select_dodge_dir(&mut w, id);
    assert_eq!(w.rng.index, 1, "gate consumed exactly one US_RndT");
    assert_eq!(w.obj(id).dir, dir::NORTH, "dir unchanged");
    assert_eq!(w.obj(id).tiley, 10);

    w.rng.index = 2; // next US_RndT = 220: passes the gate
    assert_eq!(RNDTABLE[3] % 5, 0);
    actors::select_dodge_dir(&mut w, id);
    assert_eq!(w.rng.index, 4, "gate plus the randomise roll");
    assert_ne!(w.obj(id).dir, dir::NODIR);
    assert_eq!(w.obj(id).distance, 0x10000);
}

#[test]
fn halve_double_clears_bit_zero() {
    let mut w = world_or_skip!();
    actors::spawn_stand(&mut w, 0, 5, 5, 0);
    let id = last(&w);
    w.obj_mut(id).speed = 0x30F;
    actors::halve_speed(&mut w, id);
    actors::double_speed(&mut w, id);
    assert_eq!(w.obj(id).speed, 0x30E);
}

#[test]
fn chaser_opens_door_and_waits() {
    let mut w = world_or_skip!();
    put_player(&mut w, 10, 3, 270);
    for x in 0..64 {
        if x != 10 {
            wall(&mut w, x, 7);
        }
    }
    w.doors.push(Door {
        tilex: 10,
        tiley: 7,
        vertical: false,
        action: DoorAction::Closed,
        ..Door::default()
    });
    w.tilemap[World::idx(10, 7)] = 0x80;
    w.set_actorat(10, 7, Occ::Mark(0x80));
    actors::spawn_stand(&mut w, 1, 10, 8, 1);
    let id = last(&w);
    actors::first_sighting(&mut w, id);
    w.new_state(id, st::SOLDIER_CHASE);
    assert!(!actors::check_line(&w, id));
    actors::call(&mut w, id, Func::TChase);
    assert_eq!(w.doors[0].action, DoorAction::Opening);
    assert_eq!(w.obj(id).distance, -1, "waiting for door 0");
    assert_eq!(w.obj(id).tiley, 7, "destination is the door tile");
    // Still opening: the actor keeps waiting without moving.
    let y = w.obj(id).y;
    actors::call(&mut w, id, Func::TChase);
    assert_eq!(w.obj(id).y, y);
    // Once open it walks on.
    w.doors[0].action = DoorAction::Open;
    actors::call(&mut w, id, Func::TChase);
    assert!(w.obj(id).y < y);
}

#[test]
fn chaser_approaches_player() {
    let mut w = world_or_skip!();
    put_player(&mut w, 32, 10, 270);
    actors::spawn_stand(&mut w, 1, 32, 30, 1);
    let id = last(&w);
    let mut best = i32::MAX;
    for _ in 0..3000 {
        run_actors(&mut w);
        let o = *w.obj(id);
        best = best.min((i32::from(o.tiley) - 10).abs());
    }
    assert_eq!(w.obj(id).flags & fl::ATTACKMODE, fl::ATTACKMODE);
    assert!(best <= 2, "closest approach {best} tiles");
    // MoveObj never lets it stand on the player.
    let o = *w.obj(id);
    let p = *w.plr();
    assert!((o.x - p.x).abs() > 0x10000 || (o.y - p.y).abs() > 0x10000 || o.x == p.x);
}

#[test]
fn kill_scores_and_drops_item() {
    let mut w = world_or_skip!();
    put_player(&mut w, 2, 2, 0);
    actors::spawn_stand(&mut w, 1, 20, 20, 0);
    let id = last(&w);
    actors::damage_actor(&mut w, id, 1000);
    let o = *w.obj(id);
    assert_eq!(o.state, st::SOLDIER_DIE);
    assert_eq!(o.flags & fl::SHOOTABLE, 0);
    assert_eq!(o.flags & fl::NONMARK, fl::NONMARK);
    assert_eq!(w.gamestate.killcount, 1);
    assert_eq!(w.actorat(20, 20), Occ::Empty);
    // The soldier's own tile was occupied when DropItem ran: first free neighbour.
    let drop = w.statics.iter().find(|s| s.item == 11).expect("item 11 dropped");
    assert_eq!((drop.tilex, drop.tiley), (19, 19));
    // All dead: the kill timer is armed with tics<<4 + 1.
    assert_eq!(w.act.kill_timer, 17);
}

#[test]
fn damage_pain_and_first_hit_doubles() {
    let mut w = world_or_skip!();
    put_player(&mut w, 2, 2, 0);
    actors::spawn_stand(&mut w, 0, 20, 20, 0);
    let id = last(&w);
    w.obj_mut(id).hitpoints = 100;
    actors::damage_actor(&mut w, id, 10);
    let o = *w.obj(id);
    assert_eq!(o.hitpoints, 80, "not in attack mode: damage doubled");
    assert_eq!(o.state, st::RAT_PAIN1, "even hit points");
    assert_eq!(o.flags & fl::ATTACKMODE, fl::ATTACKMODE);
    actors::damage_actor(&mut w, id, 3);
    assert_eq!(w.obj(id).hitpoints, 77);
    assert_eq!(w.obj(id).state, st::RAT_PAIN);
}

#[test]
fn swamp_creature_immune_while_chasing() {
    let mut w = world_or_skip!();
    actors::spawn_special(&mut w, class::SWAMP, 20, 20);
    let id = last(&w);
    w.obj_mut(id).flags |= fl::ATTACKMODE;
    w.new_state(id, st::SWAMP_CHASE[2]);
    let hp = w.obj(id).hitpoints;
    actors::damage_actor(&mut w, id, 50);
    assert_eq!(w.obj(id).hitpoints, hp);
}

#[test]
fn grenade_blasts_wall() {
    let mut w = world_or_skip!();
    put_player(&mut w, 10, 12, 90); // facing north
    wall(&mut w, 10, 8);
    wall(&mut w, 11, 8);
    w.plane1[8 * 64 + 9] = 0x7D; // protected
    wall(&mut w, 9, 8);
    actors::spawn_grenade(&mut w, None);
    let g = last(&w);
    assert_eq!(w.obj(g).obclass, class::GRENADE);
    for _ in 0..200 {
        if w.obj(g).state != st::GRENADE_FLY {
            break;
        }
        actors::t_projectile(&mut w, g);
    }
    assert_eq!(w.obj(g).state, st::GRENADE_EXPLODE);
    assert_eq!(w.tile(10, 8), 0, "blocking wall destroyed");
    // Cleared, unless the 1-in-20 roll set the rubble on fire.
    match w.actorat(10, 8) {
        Occ::Empty => {}
        Occ::Obj(f) => assert_eq!(w.obj(f).obclass, class::FIRE),
        other => panic!("blocking wall left {other:?}"),
    }
    assert_eq!(w.tile(11, 8), 0x4D, "neighbour scorched");
    assert_eq!(w.tile(9, 8), 1, "info 0x7D protects");
    assert!(w.statics.iter().any(|s| (s.tilex, s.tiley) == (10, 8) && s.shape != -1));
    assert!(w.events.contains(&SimEvent::Shake(8)));
    assert_eq!(w.act.tremor, 10);
}

#[test]
fn grenade_splash_kills_soldier_in_blast() {
    let mut w = world_or_skip!();
    put_player(&mut w, 30, 30, 0); // facing east
    wall(&mut w, 36, 30);
    actors::spawn_stand(&mut w, 1, 35, 31, 0);
    let s = last(&w);
    actors::spawn_grenade(&mut w, None);
    let g = last(&w);
    for _ in 0..200 {
        if w.obj(g).state != st::GRENADE_FLY {
            break;
        }
        actors::t_projectile(&mut w, g);
    }
    // In the 8×8 box of the explosion every shootable actor dies.
    assert_eq!(w.obj(s).state, st::SOLDIER_DIE);
    assert_eq!(w.gamestate.killcount, 1);
}

#[test]
fn fire_ignites_actor_it_walks_into() {
    let mut w = world_or_skip!();
    put_player(&mut w, 2, 2, 0);
    actors::spawn_stand(&mut w, 1, 21, 20, 0);
    let soldier = last(&w);
    assert!(actors::spawn_fire_actor(&mut w, 20, 20));
    let fire = last(&w);
    w.obj_mut(fire).dir = dir::EAST;
    assert!(actors::is_burnable(&w, 21, 20));
    assert!(!actors::try_walk(&mut w, fire), "fire does not move into the soldier");
    assert_eq!(w.obj(soldier).state, st::SOLDIER_DIE);
    let flames = last(&w);
    assert_eq!(w.obj(flames).obclass, class::FLAME);
    assert_eq!(w.obj(flames).state, st::FLAMES);
}

#[test]
fn ignite_turns_bonus_statics_into_fire() {
    let mut w = world_or_skip!();
    w.place_item_type(11, 15, 15);
    assert!(actors::is_burnable(&w, 15, 15));
    let objs = w.objcount;
    actors::ignite(&mut w, 15, 15);
    assert_eq!(w.objcount, objs + 1);
    assert_eq!(w.obj(last(&w)).obclass, class::FIRE);
    assert_eq!(w.statics[0].shape, -1);
}

#[test]
fn boss_dead_wins() {
    let mut w = world_or_skip!();
    actors::spawn_special(&mut w, class::BOSS, 30, 30);
    let id = last(&w);
    actors::call(&mut w, id, Func::TBossDead);
    assert_eq!(w.playstate, ex::VICTORIOUS);
}

#[test]
fn slime_ball_aims_at_player() {
    let mut w = world_or_skip!();
    put_player(&mut w, 20, 10, 0);
    actors::spawn_special(&mut w, class::SWAMP, 20, 20);
    let id = last(&w);
    w.obj_mut(id).flags = 0;
    actors::call(&mut w, id, Func::ASlimeThrow);
    assert_eq!(w.obj(id).flags & fl::SHOOTABLE, fl::SHOOTABLE);
    let ball = last(&w);
    let b = *w.obj(ball);
    assert_eq!(b.obclass, class::SLIME);
    assert_eq!(b.angle, 90, "straight north");
    assert_eq!(b.target, Some(w.player));
}

/// Consequence of the code as ported (LIKELY, needs a trace): the thrower is
/// shootable again and marked on its tile when the ball's first
/// `T_Projectile` runs in the same PlayLoop pass, so `ProjectileTryMove`
/// stops the ball at once and it splashes at the creature's feet.
#[test]
fn slime_ball_splashes_on_its_thrower() {
    let mut w = world_or_skip!();
    put_player(&mut w, 20, 10, 0);
    actors::spawn_special(&mut w, class::SWAMP, 20, 20);
    let id = last(&w);
    w.obj_mut(id).flags = fl::ATTACKMODE;
    w.new_state(id, 0x25DE); // A_SlimeThrow frame
    w.obj_mut(id).ticcount = 1;
    run_actors(&mut w);
    let ball = last(&w);
    assert_eq!(w.obj(ball).obclass, class::SLIME);
    assert_eq!(w.obj(ball).state, st::SLIME_SPLASH);
}

/// Every enemy class runs through its state machine for a while without
/// panicking (overflow checks are on in tests).
#[test]
fn smoke_all_classes() {
    for difficulty in [0, 2, 3] {
        let mut w = world_or_skip!();
        w.gamestate.difficulty = difficulty;
        put_player(&mut w, 32, 32, 0);
        for x in 0..64 {
            wall(&mut w, x, 0);
            wall(&mut w, x, 63);
            wall(&mut w, 0, x);
            wall(&mut w, 63, x);
        }
        let mut y = 4;
        for which in [0u16, 1, 4, 5, 6, 7, 8, 9, 12] {
            actors::spawn_stand(&mut w, which, 8, y, which % 4);
            actors::spawn_patrol(&mut w, which.min(4), 56, y, 2, false);
            y += 5;
        }
        actors::spawn_patrol(&mut w, 3, 40, 40, 1, false);
        for (i, c) in [13u16, 14, 16, 17, 22, 23].into_iter().enumerate() {
            actors::spawn_special(&mut w, c, 20 + 4 * i as u16, 50);
        }
        actors::spawn_fire_actor(&mut w, 30, 20);
        w.place_item_type(11, 31, 20);
        for t in 0..2000 {
            w.tics = 1 + (t % 3) as i16;
            run_actors(&mut w);
            if t % 97 == 0 {
                actors::spawn_grenade(&mut w, None);
                actors::spawn_player_flame(&mut w, None);
                w.obj_mut(w.player).angle = ((t * 7) % 360) as i16;
            }
            if t % 301 == 0 {
                actors::spawn_explosion_ahead(&mut w);
            }
        }
        let sounds = w.events.iter().filter(|e| matches!(e, SimEvent::SoundAt { .. })).count();
        eprintln!(
            "difficulty {difficulty}: kills {} objects {} sounds {sounds} rng {}",
            w.gamestate.killcount, w.objcount, w.rng.index
        );
        assert!(w.gamestate.killcount > 0, "grenades and fire kill something");
        assert!(sounds > 0);
        assert!(
            !w.events.iter().any(|e| matches!(e, SimEvent::Fault(_))),
            "{:?}",
            w.events.iter().filter(|e| matches!(e, SimEvent::Fault(_))).collect::<Vec<_>>()
        );
    }
}
