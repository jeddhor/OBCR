//! Push walls (`PushWall` 2281:0ce2, `MovePWalls` 2281:0e58; LEVEL_CODES
//! §4.7) on a synthetic map: no game data needed.

use std::sync::Arc;

use obc_core::trig::Tables;
use obc_sim::data::GameData;
use obc_sim::world::{AREATILE, MAPSIZE, Occ, World};

const WALL: u8 = 22;
const PUSH: u16 = 98;

/// A walled 64×64 room, every floor cell in area 0, a push wall at (10, 10)
/// and the player at (5, 5).
fn world() -> World {
    let mut w = World::new(Arc::new(GameData::default()), Arc::new(Tables::build()));
    for x in 0..MAPSIZE {
        for y in 0..MAPSIZE {
            let edge = x == 0 || y == 0 || x == MAPSIZE - 1 || y == MAPSIZE - 1;
            let i = x * MAPSIZE + y;
            if edge {
                w.tilemap[i] = WALL;
                w.actorat[i] = Occ::Mark(u16::from(WALL));
                w.plane0[y * MAPSIZE + x] = u16::from(WALL);
            } else {
                w.plane0[y * MAPSIZE + x] = AREATILE;
            }
        }
    }
    w.tilemap[10 * MAPSIZE + 10] = WALL;
    w.actorat[10 * MAPSIZE + 10] = Occ::Mark(u16::from(WALL));
    w.plane1[10 * MAPSIZE + 10] = PUSH;
    let p = w.player;
    let o = w.obj_mut(p);
    o.tilex = 5;
    o.tiley = 5;
    w.tics = 1;
    w
}

fn tile(w: &World, x: usize, y: usize) -> u8 {
    w.tilemap[x * MAPSIZE + y]
}

/// Frames until the wall stops (one tic each).
fn run(w: &mut World) -> u32 {
    let mut frames = 0;
    while w.pwall.state != 0 {
        w.move_pwalls();
        frames += 1;
        assert!(frames < 1000, "the wall never stopped");
    }
    frames
}

#[test]
fn moves_exactly_two_cells_at_double_speed() {
    let mut w = world();
    w.push_wall(10, 10, 1); // east
    assert_eq!(tile(&w, 10, 10), WALL | 0x80, "moving tile flagged");
    assert_eq!(tile(&w, 11, 10), WALL, "next cell solid at once");
    // State 1 + 2 per tic passes 0x100 after 128 tics.
    assert_eq!(run(&mut w), 128);
    assert_eq!(tile(&w, 10, 10), 0);
    assert_eq!(tile(&w, 11, 10), 0);
    assert_eq!(tile(&w, 12, 10), WALL, "rests two cells on");
    assert_eq!(tile(&w, 13, 10), 0, "never a third cell");
    // The vacated cells take the player's area; the marker does not stay.
    assert_eq!(w.plane0[10 * MAPSIZE + 11], AREATILE);
    for x in 10..=13 {
        assert_eq!(w.plane1[10 * MAPSIZE + x], 0, "no push-wall marker at x = {x}");
    }
}

#[test]
fn a_resting_wall_cannot_be_pushed_again() {
    let mut w = world();
    w.push_wall(10, 10, 1);
    run(&mut w);
    // Cmd_Use pushes only cells whose plane 1 is 98.
    assert_ne!(w.plane1[10 * MAPSIZE + 12], PUSH);
}

#[test]
fn the_player_stops_it() {
    let mut w = world();
    let p = w.player;
    w.obj_mut(p).tilex = 12; // standing two cells east
    w.obj_mut(p).tiley = 10;
    w.push_wall(10, 10, 1);
    run(&mut w);
    assert_eq!(tile(&w, 11, 10), WALL, "stops one cell on");
    assert_eq!(tile(&w, 12, 10), 0, "never onto the player");
}
