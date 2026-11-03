//! Healing stations (ghidra 2f4d:064a, 06c9, 06ec; docs/re/PLAYER.md §9).
//! Unreachable in OBC-A, so these drive the ported routines directly.

mod player_support;

use obc_sim::player::{self, HEALING, MAXSTATIONS, TURNING};
use obc_sim::world::{Occ, World};
use player_support::*;

fn station_world() -> World {
    let mut w = room_world();
    start(&mut w, 1, 12, 12, 1); // facing east, angle 0
    w.gamestate.health = 60;
    w.gamestate.maxhealth = 100;
    w.tics = 2;
    w
}

#[test]
fn idle_without_flags() {
    let mut w = station_world();
    player::spawn_heal_station(&mut w, 12, 12, 0);
    let rng = w.rng;
    player::heal_stations(&mut w);
    assert_eq!(w.gamestate.health, 60);
    assert_eq!(w.ply.stations.list[0].pool, 50);
    assert_eq!(w.ply.stations.list[0].rate, 0, "nothing is written while idle");
    assert_eq!(w.rng, rng, "no US_RndT");
}

#[test]
fn spawn_sets_pools_and_keeps_at_most_fifteen() {
    let mut w = station_world();
    for kind in 0..4 {
        player::spawn_heal_station(&mut w, kind as i16, 7, kind);
    }
    let pools: Vec<i16> = w.ply.stations.list[..4].iter().map(|s| s.pool).collect();
    assert_eq!(pools, [50, 100, 200, 300]);
    assert_eq!((w.ply.stations.list[2].x, w.ply.stations.list[2].y), (2, 7));
    // An unknown kind keeps whatever the record held.
    w.ply.stations.list[4].pool = 77;
    player::spawn_heal_station(&mut w, 1, 1, 9);
    assert_eq!(w.ply.stations.list[4].pool, 77);
    for _ in 0..20 {
        player::spawn_heal_station(&mut w, 1, 1, 0);
    }
    assert_eq!(w.ply.stations.count as usize, MAXSTATIONS - 1);
}

#[test]
fn turns_then_closes_the_wall_then_heals_from_the_pool() {
    let mut w = station_world();
    // An open barrier two tiles east of the start tile.
    w.spawn_anim_wall(14, 12, 1);
    w.animwalls[0].state = 0;
    w.set_actorat(14, 12, Occ::Empty);
    player::spawn_heal_station(&mut w, 12, 12, 0); // pool 50
    player::start_heal_station(&mut w, 3, 0, 12, 12); // face angle 0
    assert_eq!(w.ply.stations.angle, 0);
    w.gamestate.itemflags |= TURNING;
    let p = w.player;
    w.obj_mut(p).angle = 5;

    player::heal_stations(&mut w);
    assert_eq!(w.plr().angle, 3);
    assert_eq!(w.ply.stations.list[0].rate, 2 << 5);
    player::heal_stations(&mut w);
    assert_eq!(w.plr().angle, 1);
    assert_eq!(w.gamestate.itemflags & TURNING, TURNING);
    player::heal_stations(&mut w); // clamps to 0 and switches to healing
    assert_eq!(w.plr().angle, 0);
    assert_eq!(w.gamestate.itemflags & (TURNING | HEALING), HEALING);
    assert_eq!((w.ply.stations.wall_x, w.ply.stations.wall_y), (14, 12));
    assert_eq!(w.animwalls[0].state, 3, "the open wall closes");
    assert_eq!(w.actorat(14, 12), Occ::Mark(1));
    assert_eq!(w.gamestate.health, 60, "no healing on the turning frame");

    for _ in 0..20 {
        player::heal_stations(&mut w);
    }
    assert_eq!(w.gamestate.health, 100);
    assert_eq!(w.ply.stations.list[0].pool, 10);
    assert_eq!(w.gamestate.itemflags & HEALING, HEALING);
    player::heal_stations(&mut w); // health is at maxhealth: done
    assert_eq!(w.gamestate.itemflags & HEALING, 0);
    assert_eq!(w.ply.stations.list[0].pool, 10);
}

#[test]
fn healing_stops_when_the_pool_runs_out() {
    let mut w = station_world();
    w.gamestate.health = 10;
    player::spawn_heal_station(&mut w, 12, 12, 0);
    w.ply.stations.list[0].pool = 3;
    w.gamestate.itemflags |= HEALING;
    player::heal_stations(&mut w);
    player::heal_stations(&mut w); // pool 1 > 0: still heals, pool goes to -1
    assert_eq!(w.gamestate.health, 14);
    assert_eq!(w.ply.stations.list[0].pool, -1);
    player::heal_stations(&mut w);
    assert_eq!(w.gamestate.health, 14);
    assert_eq!(w.gamestate.itemflags & HEALING, 0);
}

#[test]
fn turning_compares_angles_without_wrapping() {
    let mut w = station_world();
    player::start_heal_station(&mut w, 0, 0, 12, 12); // face 270
    w.gamestate.itemflags |= TURNING;
    let p = w.player;
    w.obj_mut(p).angle = 350;
    player::heal_stations(&mut w);
    assert_eq!(w.plr().angle, 348, "turns the long way down to 270");
}

#[test]
fn the_frame_runs_the_stations_after_the_animated_walls() {
    let mut w = station_world();
    player::spawn_heal_station(&mut w, 12, 12, 1);
    w.gamestate.itemflags |= HEALING;
    w.frame(&cmd(), 2);
    assert_eq!(w.gamestate.health, 62);
    assert_eq!(w.ply.stations.list[0].pool, 98);
}

#[test]
fn level_start_clears_the_low_item_flags() {
    let mut w = station_world();
    w.gamestate.itemflags = 0xFFFF;
    player::setup_level_player_state(&mut w);
    assert_eq!(w.gamestate.itemflags, 0xFF80);
}

#[test]
fn use_reverses_a_closing_animated_wall() {
    let mut w = station_world();
    w.spawn_anim_wall(14, 12, 1);
    w.animwalls[0].state = 3; // closing: actorat holds the marker
    w.set_actorat(14, 12, Occ::Mark(1));
    w.operate_anim_wall(14, 12);
    assert_eq!(w.animwalls[0].state, 2, "OperateAnimWall: closing -> opening");
    // Opening with the wall marker still in actorat: nothing happens.
    w.operate_anim_wall(14, 12);
    assert_eq!(w.animwalls[0].state, 2);
}
