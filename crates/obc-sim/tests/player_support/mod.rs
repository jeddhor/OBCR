//! Shared fixtures for the player tests: a hand-built `GameData` (player
//! states and the `attackinfo` table transcribed from docs/re/PLAYER.md §3.9)
//! and a small walled room.
#![allow(dead_code)]

use std::collections::BTreeMap;
use std::sync::Arc;

use obc_core::trig::Tables;
use obc_sim::data::{AttackStep, Func, GameData, S_ATTACK, S_PLAYER, State};
use obc_sim::events::SimEvent;
use obc_sim::player;
use obc_sim::world::{AREATILE, MAPSIZE, Occ, TicCmd, World};

/// `attackinfo` (DS:0x0F66), entries 0–4 of each weapon: {tics, attack, frame}.
pub const ATTACKINFO: [[(i8, i8, i8); 5]; 5] = [
    [(6, 0, 0), (6, 2, 1), (6, 0, 2), (56, 0, 3), (36, -1, 4)],
    [(6, 1, 0), (6, 2, 1), (6, 2, 2), (6, 3, 3), (6, -1, 4)],
    [(6, 1, 0), (6, 2, 1), (6, 2, 2), (6, 3, 3), (6, -1, 4)],
    [(6, 0, 0), (6, 5, 1), (6, 1, 2), (6, 4, 3), (6, -1, 4)],
    [(6, 0, 0), (6, 0, 1), (20, 1, 2), (6, 3, 3), (6, -1, 4)],
];

pub fn game_data() -> GameData {
    let mut states = BTreeMap::new();
    let st =
        |think| State { rotate: 0, shape: 0, tics: 0, think: Some(think), action: None, next: 0 };
    states.insert(S_PLAYER, st(Func::TPlayer));
    states.insert(S_ATTACK, st(Func::TAttack));
    let mut attackinfo = [[AttackStep::default(); 15]; 5];
    for (w, row) in ATTACKINFO.iter().enumerate() {
        for (k, &(tics, attack, frame)) in row.iter().enumerate() {
            attackinfo[w][k] = AttackStep { tics, attack, frame };
        }
    }
    GameData { states, attackinfo, statinfo: Vec::new() }
}

/// Room interior (inclusive) in a map of solid walls.
pub const ROOM: (i32, i32, i32, i32) = (10, 10, 14, 14);
/// Area of the room floor.
pub const ROOM_AREA: u16 = 4;

/// A world whose only open space is [`ROOM`], floor area [`ROOM_AREA`].
pub fn room_world() -> World {
    let mut w = World::new(Arc::new(game_data()), Arc::new(Tables::build()));
    for x in 0..MAPSIZE as i32 {
        for y in 0..MAPSIZE as i32 {
            set_wall(&mut w, x, y, 1);
        }
    }
    let (x0, y0, x1, y1) = ROOM;
    for x in x0..=x1 {
        for y in y0..=y1 {
            set_floor(&mut w, x, y, ROOM_AREA);
        }
    }
    w
}

pub fn set_wall(w: &mut World, x: i32, y: i32, tile: u8) {
    let i = World::idx(x, y);
    w.tilemap[i] = tile;
    w.actorat[i] = Occ::Mark(u16::from(tile));
    w.set_mapspot(x, y, 0, u16::from(tile));
}

pub fn set_floor(w: &mut World, x: i32, y: i32, area: u16) {
    let i = World::idx(x, y);
    w.tilemap[i] = 0;
    w.actorat[i] = Occ::Empty;
    w.set_mapspot(x, y, 0, AREATILE + area);
}

/// New game at `difficulty`, player spawned at (x, y) facing `dir` (0 N, 1 E, 2 S, 3 W).
pub fn start(w: &mut World, difficulty: i16, x: u16, y: u16, dir: u16) {
    player::new_game(w, difficulty);
    player::spawn_player(w, x, y, dir);
    player::play_loop_start(w);
    w.tics = 1;
}

/// One PlayLoop frame as far as the player is concerned.
pub fn frame(w: &mut World, cmd: &TicCmd) {
    player::apply_controls(w, cmd);
    player::pre_actors(w);
    w.move_doors();
    let p = w.player;
    w.do_actor(p, 0);
    player::post_actors(w);
    player::check_keys(w);
}

pub fn cmd() -> TicCmd {
    TicCmd::default()
}

pub fn with_button(mut c: TicCmd, b: usize) -> TicCmd {
    c.buttons[b] = true;
    c
}

/// Drains the events, returning them.
pub fn take_events(w: &mut World) -> Vec<SimEvent> {
    std::mem::take(&mut w.events)
}

pub fn messages(events: &[SimEvent]) -> Vec<String> {
    events
        .iter()
        .filter_map(|e| if let SimEvent::Message(m) = e { Some(m.clone()) } else { None })
        .collect()
}

pub fn sounds(events: &[SimEvent]) -> Vec<u16> {
    events.iter().filter_map(|e| if let SimEvent::Sound(s) = e { Some(*s) } else { None }).collect()
}
