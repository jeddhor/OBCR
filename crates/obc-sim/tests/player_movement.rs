//! Player spawning, movement, collision and the use key (docs/re/PLAYER.md §3.3–3.6).

mod player_support;

use obc_sim::data::S_PLAYER;
use obc_sim::events::SimEvent;
use obc_sim::world::{AREATILE, Door, DoorAction, NUMAREAS, Occ, World, bt, class, ex, fl};
use player_support::*;

const PLAYERSIZE: i32 = 0x5800;

#[test]
fn spawn_sets_position_angle_state_and_area() {
    let mut w = room_world();
    start(&mut w, 1, 12, 13, 1);
    let p = *w.plr();
    assert_eq!(w.player, 0);
    assert_eq!((p.tilex, p.tiley), (12, 13));
    assert_eq!((p.x, p.y), ((12 << 16) + 0x8000, (13 << 16) + 0x8000));
    assert_eq!(p.angle, 0, "code 20 faces east");
    assert_eq!(p.state, S_PLAYER);
    assert_eq!(p.obclass, class::PLAYER);
    assert_eq!(p.flags, fl::NEVERMARK);
    assert_eq!(p.areanumber as u16, ROOM_AREA);
    let mut areas = [false; NUMAREAS];
    areas[ROOM_AREA as usize] = true;
    assert_eq!(w.areabyplayer, areas);

    for (dir, angle) in [(0, 90), (2, 270), (3, 180)] {
        let mut w = room_world();
        start(&mut w, 1, 12, 12, dir);
        assert_eq!(w.plr().angle, angle);
    }
}

#[test]
fn random_game_type_moves_the_player_only_when_boxed_in() {
    // One neighbour free: the neighbour is chosen, then overwritten with the
    // original cell (208c:143d), and no random number is drawn.
    let mut w = room_world();
    w.actorat[World::idx(12, 12)] = Occ::Mark(1);
    w.actorat[World::idx(11, 12)] = Occ::Mark(1);
    start(&mut w, 3, 12, 12, 1);
    assert_eq!((w.plr().tilex, w.plr().tiley), (12, 12));
    assert_eq!(w.rng.index, 0);

    // All four neighbours occupied: RandomNear picks a free cell.
    let mut w = room_world();
    for x in 1..=40 {
        for y in 1..=40 {
            set_floor(&mut w, x, y, ROOM_AREA);
        }
    }
    for (x, y) in [(12, 12), (11, 12), (13, 12), (12, 11), (12, 13)] {
        w.actorat[World::idx(x, y)] = Occ::Mark(1);
    }
    start(&mut w, 3, 12, 12, 1);
    let p = *w.plr();
    assert!(w.rng.index > 0, "RandomNear draws US_RndT");
    assert!(w.actorat(i32::from(p.tilex), i32::from(p.tiley)).is_empty());
    assert_ne!((p.tilex, p.tiley), (12, 12));
}

#[test]
fn walking_east_stops_at_the_wall() {
    let mut w = room_world();
    start(&mut w, 1, 12, 12, 1);
    let mut c = cmd();
    c.controly = -70; // RUNMOVE at 1 tic
    for _ in 0..200 {
        frame(&mut w, &c);
    }
    let p = *w.plr();
    // The wall is tile 15: the box edge may not reach it.
    assert!(p.x + PLAYERSIZE < 15 << 16, "x = {:#x}", p.x);
    assert!(p.x + PLAYERSIZE > (15 << 16) - 70 * 150, "player should touch the wall");
    assert_eq!(p.tilex, 14);
    assert_eq!(p.y, (12 << 16) + 0x8000, "no drift");
    assert!(w.ply.walkcount > 0, "hand bob advances while walking");
    assert_eq!(w.thrustspeed, 70 * 150);
}

#[test]
fn turning_and_strafing() {
    let mut w = room_world();
    start(&mut w, 1, 12, 12, 1);
    // controlx 100 → anglefrac 100 → 5 units: angle 0 - 5 = 355.
    let mut c = cmd();
    c.controlx = 100;
    frame(&mut w, &c);
    assert_eq!(w.plr().angle, 355);
    assert_eq!(w.anglefrac, 0);
    // Partial units accumulate in anglefrac.
    c.controlx = 30;
    frame(&mut w, &c);
    assert_eq!((w.plr().angle, w.anglefrac), (354, 10));
    // The clamp: at most 100·tics per frame.
    c.controlx = 1000;
    frame(&mut w, &c);
    assert_eq!(w.controlx, 100);

    // Strafe right (controlx > 0) at angle 0 moves toward angle 270 (south, +y).
    let mut w = room_world();
    start(&mut w, 1, 12, 12, 1);
    let mut c = with_button(cmd(), bt::STRAFE);
    c.controlx = 35;
    let y0 = w.plr().y;
    frame(&mut w, &c);
    assert_eq!(w.plr().angle, 0, "strafing does not turn");
    assert!(w.plr().y > y0);
    assert_eq!(w.plr().x, (12 << 16) + 0x8000);
}

#[test]
fn diagonal_move_slides_along_the_wall() {
    let mut w = room_world();
    start(&mut w, 1, 14, 12, 1);
    w.obj_mut(0).angle = 45; // north-east into the east wall
    let mut c = cmd();
    c.controly = -70;
    let y0 = w.plr().y;
    for _ in 0..20 {
        frame(&mut w, &c);
    }
    let p = *w.plr();
    assert!(p.x + PLAYERSIZE < 15 << 16);
    assert!(p.y < y0, "ClipMove keeps the y part of the move");
}

#[test]
fn actors_block_unless_flagged() {
    let mut w = room_world();
    start(&mut w, 1, 11, 12, 1);
    let a = w.get_new_actor().unwrap();
    {
        let o = w.obj_mut(a);
        o.obclass = class::SOLDIER;
        o.flags = fl::SHOOTABLE;
        o.x = (13 << 16) + 0x8000;
        o.y = (12 << 16) + 0x8000;
        o.tilex = 13;
        o.tiley = 12;
    }
    w.actorat[World::idx(13, 12)] = Occ::Obj(a);
    let mut c = cmd();
    c.controly = -70;
    for _ in 0..100 {
        frame(&mut w, &c);
    }
    // Blocked within MINACTORDIST (0x10000) of the actor's centre.
    assert!(w.plr().x < w.obj(a).x - 0x10000 + 70 * 150);
    assert!(w.plr().x >= w.obj(a).x - 0x10000 - 70 * 150);

    // flags2 bit 1 (rats, saucers, allies) does not block.
    w.obj_mut(a).flags2 = fl::NOBLOCK;
    for _ in 0..30 {
        frame(&mut w, &c);
    }
    assert!(w.plr().x > w.obj(a).x - 0x10000);
}

#[test]
fn damaging_wall_hurts_when_faced() {
    let mut w = room_world();
    start(&mut w, 1, 14, 12, 1);
    set_wall(&mut w, 15, 12, 0x49);
    w.gamestate.armor = 0;
    let mut c = cmd();
    c.controly = -35;
    // The first step still fits (0xED800 + 5250 < 0xF0000).
    frame(&mut w, &c);
    assert_eq!(w.gamestate.health, 400);
    let h = w.gamestate.health;
    let rng = w.rng.index;
    take_events(&mut w);
    frame(&mut w, &c);
    // TakeDamage(5): difficulty 1 → ×1 then ×2 = 10 per blocked attempt, and
    // ClipMove makes two blocked attempts (full move, x only) before the
    // y-only move (ymove = 0) succeeds.
    assert_eq!(w.gamestate.health, h - 20);
    let draws = w.rng.index.wrapping_sub(rng);
    assert!(draws >= 2, "one US_RndT per splat roll, more if a splat lands ({draws})");
    let ev = take_events(&mut w);
    assert_eq!(ev.iter().filter(|e| matches!(e, SimEvent::DamageFlash(6))).count(), 2);

    // Backing into it while facing west (angle 180): the cascade's
    // `x > gx` and `y < gy` tests are false for a tile east on the same row.
    let mut w = room_world();
    start(&mut w, 1, 14, 12, 3);
    set_wall(&mut w, 15, 12, 0x49);
    let mut c = cmd();
    c.controly = 70;
    let h = w.gamestate.health;
    for _ in 0..20 {
        frame(&mut w, &c);
    }
    assert_eq!(w.gamestate.health, h);
    assert!(w.plr().x + PLAYERSIZE > (15 << 16) - 70 * 100, "pressed against the wall");
}

#[test]
fn area_changes_follow_the_player() {
    let mut w = room_world();
    // Columns 13–14 are area 5.
    for y in 10..=14 {
        set_floor(&mut w, 13, y, 5);
        set_floor(&mut w, 14, y, 5);
    }
    start(&mut w, 1, 11, 12, 1);
    assert!(w.areabyplayer[ROOM_AREA as usize]);
    let mut c = cmd();
    c.controly = -70;
    for _ in 0..30 {
        frame(&mut w, &c);
    }
    assert_eq!(w.plr().areanumber, 5);
    assert!(w.areabyplayer[5]);
    assert!(!w.areabyplayer[ROOM_AREA as usize]);
    // Plane-0 codes outside 140..161 never change the area.
    w.set_mapspot(14, 12, 0, AREATILE - 1);
    for _ in 0..30 {
        frame(&mut w, &c);
    }
    assert_eq!(w.plr().areanumber, 5);
}

fn door_world() -> World {
    let mut w = room_world();
    // A vertical door in the east wall at (15, 12), leading to a cell in area 5.
    set_floor(&mut w, 16, 12, 5);
    let i = World::idx(15, 12);
    w.tilemap[i] = 0x80;
    w.actorat[i] = Occ::Mark(0x80);
    w.set_mapspot(15, 12, 0, AREATILE + ROOM_AREA);
    w.doors.push(Door {
        tilex: 15,
        tiley: 12,
        vertical: true,
        lock: 0,
        action: DoorAction::Closed,
        ticcount: 0,
        kind: 0,
        position: 0,
    });
    w
}

#[test]
fn use_opens_and_closes_doors_on_the_edge() {
    let mut w = door_world();
    start(&mut w, 1, 14, 12, 1);
    let use_cmd = with_button(cmd(), bt::USE);
    frame(&mut w, &use_cmd);
    assert_eq!(w.doors[0].action, DoorAction::Opening);
    // Holding use does nothing more.
    frame(&mut w, &use_cmd);
    frame(&mut w, &use_cmd);
    assert_eq!(w.doors[0].action, DoorAction::Opening);
    // 64 tics later the door is open and passable.
    for _ in 0..70 {
        frame(&mut w, &cmd());
    }
    assert_eq!(w.doors[0].action, DoorAction::Open);
    assert!(w.actorat(15, 12).is_empty());
    // Press again: OperateDoor closes an open door.
    frame(&mut w, &use_cmd);
    assert_eq!(w.doors[0].action, DoorAction::Closing);
}

#[test]
fn exits_need_half_the_kills() {
    let mut w = room_world();
    start(&mut w, 1, 14, 12, 1);
    set_wall(&mut w, 15, 12, 0x73);
    w.gamestate.killtotal = 5;
    w.gamestate.killcount = 1;
    frame(&mut w, &with_button(cmd(), bt::USE));
    assert_eq!(w.playstate, ex::STILLPLAYING);
    assert_eq!(messages(&take_events(&mut w)), ["UNFINISHED BUSINESS"]);
    assert_eq!(w.ply.exitkind, 1);

    w.gamestate.killcount = 2; // 5 >> 1 = 2
    frame(&mut w, &cmd());
    frame(&mut w, &with_button(cmd(), bt::USE));
    assert_eq!(w.playstate, ex::COMPLETED);

    // Map 39: exits do nothing.
    let mut w = room_world();
    start(&mut w, 1, 14, 12, 1);
    set_wall(&mut w, 15, 12, 0x71);
    w.gamestate.mapon = 39;
    frame(&mut w, &with_button(cmd(), bt::USE));
    assert_eq!(w.playstate, ex::STILLPLAYING);
    assert_eq!(w.ply.exitkind, 0);
}

#[test]
fn medic_wall_heals_once_up_to_maxhealth() {
    let mut w = room_world();
    start(&mut w, 1, 14, 12, 1);
    set_wall(&mut w, 15, 12, 0x58);
    w.set_mapspot(15, 12, 1, 18);
    w.gamestate.health = 90;
    frame(&mut w, &with_button(cmd(), bt::USE));
    assert_eq!(w.gamestate.health, 100, "HealSelf(25) capped at maxhealth 100");
    assert_eq!(w.mapspot(15, 12, 1), 0, "plane 1 cleared");
    assert_eq!(messages(&take_events(&mut w)), ["Medic Pack"]);
    // At maxhealth nothing happens.
    frame(&mut w, &cmd());
    frame(&mut w, &with_button(cmd(), bt::USE));
    assert!(messages(&take_events(&mut w)).is_empty());
}

#[test]
fn generator_switch_toggles() {
    let mut w = room_world();
    start(&mut w, 1, 14, 12, 1);
    set_wall(&mut w, 15, 12, 0x60);
    frame(&mut w, &with_button(cmd(), bt::USE));
    assert_eq!(w.tile(15, 12), 0x5F);
    let ev = take_events(&mut w);
    assert!(ev.contains(&SimEvent::Lighting(1)));
    assert_eq!(messages(&ev), ["GENERATORS ON"]);
    assert_eq!(sounds(&ev), [84]);
    frame(&mut w, &cmd());
    frame(&mut w, &with_button(cmd(), bt::USE));
    assert_eq!(w.tile(15, 12), 0x60);
    assert!(take_events(&mut w).contains(&SimEvent::Lighting(2)));
}

#[test]
fn switch_walls_play_their_sound_and_clear_plane_one() {
    let mut w = room_world();
    start(&mut w, 1, 14, 12, 1);
    set_wall(&mut w, 15, 12, 0x1D);
    w.set_mapspot(15, 12, 1, 18);
    frame(&mut w, &with_button(cmd(), bt::USE));
    assert_eq!(sounds(&take_events(&mut w)), [80]);
    assert_eq!(w.mapspot(15, 12, 1), 0);
}

#[test]
fn option_keys_and_map_overlay() {
    let mut w = room_world();
    start(&mut w, 1, 12, 12, 1);
    assert_eq!(w.gamestate.options, 3);
    frame(&mut w, &with_button(cmd(), bt::TOGGLE_S));
    assert_eq!(w.gamestate.options, 2);
    frame(&mut w, &with_button(cmd(), bt::TOGGLE_S)); // held: no toggle
    assert_eq!(w.gamestate.options, 2);
    let tab = with_button(cmd(), bt::MAP);
    frame(&mut w, &tab);
    assert_eq!(w.ply.map_overlay, 1);
    frame(&mut w, &cmd());
    frame(&mut w, &tab);
    assert_eq!(w.ply.map_overlay, -1);
    frame(&mut w, &cmd());
    frame(&mut w, &tab);
    assert_eq!(w.ply.map_overlay, -1, "stays until the next level");
}

#[test]
fn knock_back_pushes_the_player_backward() {
    let mut w = room_world();
    start(&mut w, 1, 12, 12, 1);
    w.ply.knockback_trigger = 10;
    let x0 = w.plr().x;
    frame(&mut w, &cmd());
    assert_eq!(w.ply.knockback_speed, 42);
    assert_eq!(w.ply.knockback_trigger, 0);
    assert_eq!(w.controly, 126);
    assert!(w.plr().x < x0, "pushed west (backward)");
    frame(&mut w, &cmd());
    assert_eq!(w.ply.knockback_speed, 39);
    for _ in 0..20 {
        frame(&mut w, &cmd());
    }
    assert_eq!(w.ply.knockback_speed, 0);
}

#[test]
fn tremors_key_toggles_with_a_message() {
    let mut w = room_world();
    start(&mut w, 1, 12, 12, 1);
    assert!(w.ply.tremors);
    let mut c = cmd();
    c.toggle_tremors = true;
    frame(&mut w, &c);
    assert!(!w.ply.tremors);
    assert_eq!(messages(&take_events(&mut w)), ["TREMORS OFF"]);
}
