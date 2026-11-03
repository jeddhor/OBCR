//! The shootable-wall search of the per-tile pass (ghidra 1b4a:1494,
//! 1b4a:1b8b, 1b4a:1ccf, 1b4a:1b36; docs/re/RENDERER.md §9.1) and the
//! overhead-map reset of `ThreeDRefresh`.

mod player_support;

use obc_sim::world::{World, bt};
use player_support::*;

/// Player at (12,12) in the 10..14 room, facing east (angle 0), Uzi out.
fn facing_east_wall() -> World {
    let mut w = room_world();
    start(&mut w, 1, 12, 12, 1);
    w.gamestate.weapon = 1;
    w
}

fn fire(w: &mut World) {
    w.buttonstate[bt::ATTACK] = true;
}

#[test]
fn chip_wall_in_front_is_hit_while_fire_is_down() {
    let mut w = facing_east_wall();
    set_wall(&mut w, 15, 12, 0x79);
    fire(&mut w);
    w.visibility_pass();
    assert!(w.ply.wallhit_active);
    assert_eq!((w.ply.wallhit_x, w.ply.wallhit_y), (15, 12));
    assert_eq!(w.ply.wallhit_count, 2);
    assert!(w.shootable_candidate(15, 12));
    assert!(!w.shootable_candidate(15, 13), "plain wall");
}

#[test]
fn needs_fire_and_a_hitscan_weapon() {
    let mut w = facing_east_wall();
    set_wall(&mut w, 15, 12, 0x79);
    w.visibility_pass();
    assert!(!w.ply.wallhit_active, "fire not down");
    fire(&mut w);
    w.gamestate.weapon = 3;
    w.visibility_pass();
    assert!(!w.ply.wallhit_active, "flame thrower");
    w.gamestate.weapon = 2;
    w.visibility_pass();
    assert!(w.ply.wallhit_active, "Galil");
}

#[test]
fn off_centre_tiles_are_rejected() {
    let mut w = facing_east_wall();
    // One tile north of the view axis: about 58 px from centerx at view
    // size 18, more than shootdelta (28).
    set_wall(&mut w, 15, 11, 0x79);
    fire(&mut w);
    w.visibility_pass();
    assert!(w.shootable_candidate(15, 11), "its neighbour (14,11) is visible");
    assert!(!w.ply.wallhit_active);
}

#[test]
fn glass_counts_when_seen_and_ties_go_to_the_first_tile() {
    let mut w = facing_east_wall();
    // Glass pane at (13,12), see-through (plane 1 104), then a chip wall
    // behind it. Both score 0 (same row as the player); x-major order picks
    // the glass.
    set_wall(&mut w, 13, 12, 0x39);
    w.set_mapspot(13, 12, 1, 0x68);
    set_wall(&mut w, 15, 12, 0x79);
    fire(&mut w);
    w.visibility_pass();
    assert!(w.spotvis[World::idx(13, 12)]);
    assert_eq!((w.ply.wallhit_x, w.ply.wallhit_y), (13, 12));
    assert_eq!(w.ply.wallhit_count, 6);
    // The broken pane (0x3C) is no longer a candidate.
    set_wall(&mut w, 13, 12, 0x3C);
    assert!(!w.shootable_candidate(13, 12));
}

#[test]
fn holding_fire_chips_a_wall_every_frame_until_it_stops_qualifying() {
    let mut w = facing_east_wall();
    set_wall(&mut w, 15, 12, 0x79);
    let fire = with_button(cmd(), bt::ATTACK);
    w.frame(&fire, 2);
    assert_eq!(w.tile(15, 12), 0x7A);
    w.frame(&fire, 2);
    assert_eq!(w.tile(15, 12), 0x7B);
    w.frame(&fire, 2);
    assert_eq!(w.tile(15, 12), 0x7B, "0x7B is not a candidate");
    assert!(!w.ply.wallhit_active);
}

#[test]
fn tab_toggles_the_overhead_map_on_and_off() {
    let mut w = facing_east_wall();
    let tab = with_button(cmd(), bt::MAP);
    w.frame(&tab, 2);
    assert_eq!(w.ply.map_overlay, 1);
    w.frame(&cmd(), 2);
    w.frame(&tab, 2);
    assert_eq!(w.ply.map_overlay, 0, "-1 is reset by the same frame's refresh");
    w.frame(&cmd(), 2);
    w.frame(&tab, 2);
    assert_eq!(w.ply.map_overlay, 1);
}
