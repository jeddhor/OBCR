//! Data-gated tests of the network game rules (docs/re/NETWORK.md): several
//! worlds on a NETLEVEL exchange packets in process, as the transport would.

use std::sync::Arc;

use obc_core::rng::Rng;
use obc_core::trig::Tables;
use obc_formats::DataDir;
use obc_formats::exe::MzExe;
use obc_formats::maps::{Compression, Map, MapArchive};
use obc_sim::data::GameData;
use obc_sim::events::SimEvent;
use obc_sim::net::{self, NetGame, Packet, ev, kind};
use obc_sim::player;
use obc_sim::world::{TicCmd, World, bt, ex};

struct Env {
    data: Arc<GameData>,
    trig: Arc<Tables>,
    maps: MapArchive,
}

fn env() -> Option<Env> {
    let dir = DataDir::open(obc_testdata::data_dir()?).ok()?;
    let exe = MzExe::parse(dir.read("COUNT.EXE").ok()?).ok()?;
    let data = Arc::new(GameData::load(&exe).ok()?);
    let maps = MapArchive::parse(
        &dir.read("MAPHEAD.BC").ok()?,
        dir.read("MAPTEMP.BC").ok()?,
        Compression::Rlew,
    )
    .ok()?;
    Some(Env { data, trig: Arc::new(Tables::build()), maps })
}

macro_rules! env_or_skip {
    () => {
        match env() {
            Some(e) => e,
            None => {
                eprintln!("SKIPPED: original game data not found (set OBC_DATA_DIR)");
                return;
            }
        }
    };
}

/// A machine in a network game of type `k`, as the setup screen leaves it.
fn machine(e: &Env, k: i16, id: u16, choice: i16, seed: u8) -> World {
    machine_on(e, net::FIRST_NETLEVEL, k, id, choice, seed)
}

fn machine_on(e: &Env, mapon: i16, k: i16, id: u16, choice: i16, seed: u8) -> World {
    let map: Map = e.maps.map(mapon as usize).unwrap().unwrap();
    let mut w = World::new(e.data.clone(), e.trig.clone());
    player::new_game(&mut w, k);
    w.rng = Rng::seeded(seed);
    w.net = Some(Box::new(NetGame::new(id, choice)));
    w.gamestate.mapon = mapon;
    w.setup_game_level(&map, mapon);
    w.start_play_loop();
    w
}

/// Delivers every queued packet to every other machine; `drop(n)` decides
/// per delivery whether it is lost.
fn exchange(ws: &mut [World], mut drop: impl FnMut(usize) -> bool) {
    let mut sent: Vec<(usize, Packet)> = Vec::new();
    for (i, w) in ws.iter_mut().enumerate() {
        let n = w.net.as_deref_mut().unwrap();
        sent.extend(n.outbox.drain(..).map(|o| (i, o.packet)));
    }
    let mut k = 0;
    for (from, p) in sent {
        for (i, w) in ws.iter_mut().enumerate() {
            if i != from {
                k += 1;
                if !drop(k) {
                    w.net.as_deref_mut().unwrap().inbox.push(p);
                }
            }
        }
    }
}

/// One frame on every machine, then delivery. Deaths respawn at once, as the
/// game screen does after the death sequence.
fn step(ws: &mut [World], cmds: &[TicCmd]) {
    for (w, c) in ws.iter_mut().zip(cmds) {
        w.frame(c, 2);
        if w.playstate == ex::DIED {
            net::respawn_after_death(w);
        }
    }
    exchange(ws, |_| false);
}

fn faults(w: &mut World) -> Vec<String> {
    w.events
        .drain(..)
        .filter_map(|e| if let SimEvent::Fault(m) = e { Some(m) } else { None })
        .collect()
}

/// A straight east-west run of `len` free floor cells, for duels.
fn corridor(w: &World, len: i32) -> Option<(i32, i32)> {
    for y in 2..62 {
        let mut run = 0;
        for x in 1..63 {
            let free = w.tile(x, y) == 0
                && w.actorat(x, y).is_empty()
                && !w.statics.iter().any(|s| i32::from(s.tilex) == x && i32::from(s.tiley) == y);
            run = if free { run + 1 } else { 0 };
            if run == len {
                return Some((x - len + 1, y));
            }
        }
    }
    None
}

fn place(w: &mut World, x: i32, y: i32, angle: i16) {
    let p = w.player;
    let area = w.plr().areanumber;
    let o = w.obj_mut(p);
    o.tilex = x as u16;
    o.tiley = y as u16;
    o.x = (x << 16) | 0x8000;
    o.y = (y << 16) | 0x8000;
    o.angle = angle;
    o.areanumber = area;
}

fn idle() -> TicCmd {
    TicCmd::default()
}

#[test]
fn players_see_each_other() {
    let e = env_or_skip!();
    let mut ws = [machine(&e, kind::NETWORK, 10, 0, 1), machine(&e, kind::NETWORK, 20, 1, 77)];
    for _ in 0..4 {
        step(&mut ws, &[idle(), idle()]);
    }
    for (i, other) in [(0usize, 20u16), (1, 10)] {
        let n = ws[i].net.as_deref().unwrap();
        assert_eq!(n.slots.len(), 1, "machine {i} knows one player");
        assert_eq!(n.slots[0].id, other);
        let puppet = n.slots[0].puppet.expect("puppet spawned");
        let them = *ws[1 - i].plr();
        let o = *ws[i].obj(puppet);
        assert_eq!((o.x, o.y), (them.x, them.y), "puppet follows the player");
        assert_eq!(ws[i].gamestate.killcount, 1, "status bar counts players");
    }
    // The Agent's puppet is an ally, Terrorist 1's a soldier.
    let p0 = ws[1].net.as_deref().unwrap().slots[0].puppet.unwrap();
    let p1 = ws[0].net.as_deref().unwrap().slots[0].puppet.unwrap();
    assert_eq!(ws[1].obj(p0).obclass, 5);
    assert_eq!(ws[0].obj(p1).obclass, 4);
    assert_eq!(ws[0].gamestate.health, 800, "network start health");
    for w in &mut ws {
        assert!(faults(w).is_empty());
    }
}

/// A duel: A shoots B in a corridor until B dies; both machines count it.
fn duel(k: i16, choices: (i16, i16), frames: u32) -> Option<[World; 2]> {
    let e = env()?;
    let mut ws = [machine(&e, k, 10, choices.0, 3), machine(&e, k, 20, choices.1, 200)];
    for _ in 0..4 {
        step(&mut ws, &[idle(), idle()]);
    }
    let (x, y) = corridor(&ws[0], 6).expect("a corridor on NETLEVEL 5");
    let fire = TicCmd { buttons: { std::array::from_fn(|b| b == bt::ATTACK) }, ..idle() };
    for _ in 0..frames {
        place(&mut ws[0], x, y, 0);
        place(&mut ws[1], x + 4, y, 180);
        step(&mut ws, &[fire, idle()]);
        if ws[0].gamestate.score > 0 {
            break;
        }
    }
    Some(ws)
}

#[test]
fn shots_kill_on_the_victims_machine_and_both_count_it() {
    let Some(mut ws) = duel(kind::NETWORK, (0, 1), 4000) else {
        eprintln!("SKIPPED: original game data not found (set OBC_DATA_DIR)");
        return;
    };
    assert_eq!(ws[0].gamestate.score, 1, "A's frag");
    let (a, b) = (ws[0].net.as_deref().unwrap(), ws[1].net.as_deref().unwrap());
    assert_eq!(a.kills[0][1], 1, "A's table: A killed slot 1");
    assert_eq!(b.kills[1][0], 1, "B's table: slot 1 killed B");
    assert_eq!(ws[1].gamestate.health, 800, "B respawned with full health");
    for w in &mut ws {
        assert!(faults(w).is_empty());
    }
}

#[test]
fn team_mates_cannot_hurt_each_other() {
    let Some(ws) = duel(kind::TEAMS, (1, 1), 600) else {
        eprintln!("SKIPPED: original game data not found (set OBC_DATA_DIR)");
        return;
    };
    assert_eq!(ws[1].gamestate.health, 800);
    assert_eq!(ws[0].gamestate.score, 0);
}

#[test]
fn opponents_in_a_team_game_can() {
    let Some(ws) = duel(kind::TEAMS, (0, 2), 4000) else {
        eprintln!("SKIPPED: original game data not found (set OBC_DATA_DIR)");
        return;
    };
    assert_eq!(ws[0].gamestate.score, 1);
}

/// A player who uses an exit takes the whole floor along: the Use is replayed
/// on every machine (358f:19fd event 3), and Cmd_Use there completes the level.
#[test]
fn a_remote_use_of_an_exit_completes_the_level_everywhere() {
    let e = env_or_skip!();
    let mut ws = [machine(&e, kind::NETWORK, 10, 0, 5), machine(&e, kind::NETWORK, 20, 2, 9)];
    for _ in 0..4 {
        step(&mut ws, &[idle(), idle()]);
    }
    let w = &ws[1];
    let found = (1..63).flat_map(|x| (1..63).map(move |y| (x, y))).find_map(|(x, y)| {
        if !(0x71..=0x72).contains(&w.tile(x, y)) {
            return None;
        }
        [(-1, 0, 0i16), (1, 0, 180), (0, 1, 90), (0, -1, 270)]
            .into_iter()
            .find(|&(dx, dy, _)| {
                w.tile(x + dx, y + dy) == 0 && w.actorat(x + dx, y + dy).is_empty()
            })
            .map(|(dx, dy, a)| (x + dx, y + dy, a))
    });
    let (x, y, angle) = found.expect("NETLEVEL 5 has a reachable exit");
    let use_ = TicCmd { buttons: std::array::from_fn(|b| b == bt::USE), ..idle() };
    let mut done = [false; 2];
    for f in 0..6 {
        place(&mut ws[1], x, y, angle);
        let c = if f == 2 { use_ } else { idle() };
        for (i, w) in ws.iter_mut().enumerate() {
            w.frame(&if i == 1 { c } else { idle() }, 2);
            done[i] |= w.playstate == ex::COMPLETED;
        }
        exchange(&mut ws, |_| false);
    }
    assert!(done[1], "B took the exit");
    assert!(done[0], "and A's floor ended with it");
}

#[test]
fn queued_events_are_not_lost() {
    let e = env_or_skip!();
    let mut a = machine(&e, kind::NETWORK, 10, 0, 1);
    a.frame(&idle(), 2);
    let base =
        Packet { mapon: 50, id: 20, x: 0x0A8000, y: 0x0A8000, choice: 1, ..Packet::default() };
    let n = a.net.as_deref_mut().unwrap();
    n.inbox.push(base);
    a.frame(&idle(), 2);
    let puppet = a.net.as_deref().unwrap().slots[0].puppet.unwrap();
    // Three packets in one frame: movement, death, movement.
    let n = a.net.as_deref_mut().unwrap();
    n.inbox.push(Packet { event: ev::MOVING, ..base });
    n.inbox.push(Packet { event: ev::DIED, target: 10, ..base });
    n.inbox.push(Packet { event: ev::MOVING, x: 0x0B8000, ..base });
    a.frame(&idle(), 2);
    a.frame(&idle(), 2);
    assert_eq!(a.gamestate.score, 1, "the death in the middle was applied");
    assert_eq!(a.net.as_deref().unwrap().kills[0][1], 1);
    a.frame(&idle(), 2);
    assert_eq!(a.obj(puppet).x, 0x0B8000, "and the newest position after it");
}

#[test]
fn level_done_forgets_players_and_reloads() {
    let e = env_or_skip!();
    let mut ws = [machine(&e, kind::NETWORK, 10, 0, 1), machine(&e, kind::NETWORK, 20, 1, 2)];
    for _ in 0..4 {
        step(&mut ws, &[idle(), idle()]);
    }
    net::level_done(&mut ws[0]);
    assert!(net::reload_level(&ws[0]));
    assert!(ws[0].net.as_deref().unwrap().slots.is_empty());
}

/// Four players, random inputs and 10 % packet loss for 6000 frames: no
/// panic, no fault, everyone keeps seeing everyone.
#[test]
fn four_player_soak_with_packet_loss() {
    let e = env_or_skip!();
    let mut ws: Vec<World> = (0..4u16)
        .map(|i| machine(&e, kind::NETWORK, 10 + i, (i % 3) as i16, 17 + i as u8 * 40))
        .collect();
    let mut seed = 0x1234_5678u32;
    let mut rnd = move || {
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        seed
    };
    let mut kills = 0;
    let duel = corridor(&ws[0], 6).expect("a corridor");
    for f in 0..6000u32 {
        // Every 600 frames players 0 and 1 duel for 300 frames.
        let dueling = f % 600 >= 300;
        if dueling {
            place(&mut ws[0], duel.0, duel.1, 0);
            place(&mut ws[1], duel.0 + 4, duel.1, 180);
        }
        let cmds: Vec<TicCmd> = (0..4)
            .map(|i| {
                if dueling && i < 2 {
                    let mut c = TicCmd::default();
                    c.buttons[bt::ATTACK] = true;
                    return c;
                }
                let r = rnd();
                let mut c = TicCmd {
                    controlx: ((r % 3) as i16 - 1) * 70,
                    controly: (((r >> 2) % 3) as i16 - 1) * 70,
                    ..TicCmd::default()
                };
                c.buttons[bt::ATTACK] = (r >> 4) % 3 == 0;
                c.buttons[bt::USE] = (r >> 6) % 11 == 0;
                if (r >> 8) % 97 == 0 {
                    c.buttons[bt::WEAPON1 + ((r >> 12) % 5) as usize] = true;
                }
                c
            })
            .collect();
        for (w, c) in ws.iter_mut().zip(&cmds) {
            w.frame(c, 2);
            if w.playstate == ex::DIED {
                net::respawn_after_death(w);
                kills += 1;
            }
            w.playstate = ex::STILLPLAYING;
            let fs = faults(w);
            assert!(fs.is_empty(), "frame {f}: {fs:?}");
        }
        exchange(&mut ws, |_| rnd() % 10 == 0);
    }
    eprintln!("deaths in the soak: {kills}");
    assert!(kills > 0, "the duels killed someone");
    for w in &ws {
        let n = w.net.as_deref().unwrap();
        assert_eq!(n.slots.len(), 3, "everyone sees everyone");
        assert!(n.slots.iter().all(|s| s.puppet.is_some()));
    }
}
