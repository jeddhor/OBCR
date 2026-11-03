//! Data-gated soak test: plays every map of the first episode (0–22) at each
//! single-player game type with scripted input, looking for panics (overflow
//! checks are on in test builds) and simulation faults.

use std::sync::Arc;

use obc_core::trig::Tables;
use obc_formats::DataDir;
use obc_formats::exe::MzExe;
use obc_formats::maps::{Compression, MapArchive};
use obc_sim::data::GameData;
use obc_sim::events::SimEvent;
use obc_sim::player;
use obc_sim::world::{TicCmd, World, bt, ex};

const FRAMES: u32 = 2500;

fn load() -> Option<(Arc<GameData>, Arc<Tables>, MapArchive)> {
    let dir = DataDir::open(obc_testdata::data_dir()?).ok()?;
    let exe = MzExe::parse(dir.read("COUNT.EXE").ok()?).ok()?;
    let data = Arc::new(GameData::load(&exe).ok()?);
    let maps = MapArchive::parse(
        &dir.read("MAPHEAD.BC").ok()?,
        dir.read("MAPTEMP.BC").ok()?,
        Compression::Rlew,
    )
    .ok()?;
    Some((data, Arc::new(Tables::build()), maps))
}

/// Wander: walk forward, turn when the position stops changing, fire in
/// bursts, press use often, switch weapons now and then.
fn script(frame: u32, stuck: u32) -> TicCmd {
    let mut c = TicCmd { controly: -70, ..TicCmd::default() };
    if stuck > 3 || frame % 97 < 12 {
        c.controlx = if (frame / 200).is_multiple_of(2) { 70 } else { -70 };
    }
    c.buttons[bt::ATTACK] = frame % 40 < 20;
    c.buttons[bt::USE] = frame.is_multiple_of(7);
    c.buttons[bt::RUN] = frame % 300 < 150;
    if frame % 500 == 250 {
        c.buttons[bt::WEAPON1 + ((frame / 500) % 5) as usize] = true;
    }
    c.ally_switch = frame % 900 == 450;
    c.ally_mode = frame % 700 == 350;
    c
}

#[test]
fn first_episode_maps_run_without_panics_or_faults() {
    let Some((data, trig, maps)) = load() else {
        eprintln!("skipped: OBC_DATA_DIR not set");
        return;
    };
    let mut faults = Vec::new();
    for difficulty in 0..=3 {
        for mapon in 0..=22i16 {
            let map = maps.map(mapon as usize).unwrap().unwrap();
            let mut w = World::new(data.clone(), trig.clone());
            player::new_game(&mut w, difficulty);
            w.gamestate.mapon = mapon;
            w.setup_game_level(&map, mapon);
            w.start_play_loop();
            w.godmode = std::env::var_os("SOAK_MORTAL").is_none();
            let (mut last, mut stuck) = ((0, 0), 0);
            let (mut moved, mut sounds) = (0u32, 0u32);
            for f in 0..FRAMES {
                w.frame(&script(f, stuck), 2);
                let p = w.plr();
                let pos = (p.x >> 12, p.y >> 12);
                stuck = if pos == last { stuck + 1 } else { 0 };
                moved += u32::from(pos != last);
                last = pos;
                for e in w.events.drain(..) {
                    if matches!(e, SimEvent::Sound(_) | SimEvent::SoundAt { .. }) {
                        sounds += 1;
                    }
                    if let SimEvent::Fault(m) = e {
                        faults.push(format!("map {mapon} type {difficulty} frame {f}: {m}"));
                    }
                }
                if f == FRAMES - 1 && difficulty == 1 {
                    let g = &w.gamestate;
                    eprintln!(
                        "map {mapon:2}: moved {moved:4} frames, kills {}/{}, health {}, sounds {sounds}, objs {}",
                        g.killcount, g.killtotal, g.health, w.objcount
                    );
                }
                if w.playstate != ex::STILLPLAYING {
                    // Exits and the like end the run; restart the level loop state.
                    w.playstate = ex::STILLPLAYING;
                }
            }
        }
    }
    faults.dedup();
    assert!(
        faults.is_empty(),
        "{} faults, first: {:#?}",
        faults.len(),
        &faults[..faults.len().min(20)]
    );
}

/// Saving mid-level, loading and continuing gives the same world as never
/// having saved (DESIGN §7.7).
#[test]
fn save_and_load_continue_identically() {
    let Some((data, trig, maps)) = load() else {
        eprintln!("skipped: OBC_DATA_DIR not set");
        return;
    };
    for mapon in [0i16, 5, 12, 22] {
        let map = maps.map(mapon as usize).unwrap().unwrap();
        let mut a = World::new(data.clone(), trig.clone());
        player::new_game(&mut a, 1);
        a.gamestate.mapon = mapon;
        a.setup_game_level(&map, mapon);
        a.start_play_loop();
        a.godmode = true;
        for f in 0..600 {
            a.frame(&script(f, 0), 2);
            a.events.clear();
        }
        let header = obc_sim::save::SaveHeader {
            format: obc_sim::save::FORMAT,
            obcr: "test".into(),
            release: "OBC-A".into(),
            mapon,
            difficulty: 1,
            name: "TEST".into(),
            timestamp: 0,
        };
        let bytes = obc_sim::save::write(&a, &header).unwrap();
        let (h, mut b) = obc_sim::save::read(&bytes, data.clone(), trig.clone()).unwrap();
        assert_eq!(h, header);
        b.loadedgame = false;
        for f in 600..1200 {
            a.frame(&script(f, 0), 2);
            b.frame(&script(f, 0), 2);
        }
        let again = obc_sim::save::write(&b, &header).unwrap();
        assert_eq!(obc_sim::save::write(&a, &header).unwrap(), again, "map {mapon} diverged");
        assert_eq!(a.rng.index, b.rng.index);
    }
}

/// Every first-episode floor has an exit reachable from the player start
/// through open floor, doors, push walls and barriers.
#[test]
fn every_first_episode_exit_is_reachable() {
    let Some((data, trig, maps)) = load() else {
        eprintln!("skipped: OBC_DATA_DIR not set");
        return;
    };
    let mut unreachable = Vec::new();
    for mapon in 0..=22i16 {
        let map = maps.map(mapon as usize).unwrap().unwrap();
        let mut w = World::new(data.clone(), trig.clone());
        player::new_game(&mut w, 1);
        w.setup_game_level(&map, mapon);
        let barrier =
            |x: i32, y: i32| w.animwalls.iter().any(|a| i32::from(a.x) == x && i32::from(a.y) == y);
        let pass = |x: i32, y: i32| {
            let t = w.tile(x, y);
            t == 0 || t & 0x80 != 0 || w.mapspot(x, y, 1) == 98 || barrier(x, y)
        };
        let p = w.plr();
        let start = (i32::from(p.tilex as i16), i32::from(p.tiley as i16));
        let mut seen = vec![false; 64 * 64];
        let mut queue = std::collections::VecDeque::from([start]);
        seen[(start.0 * 64 + start.1) as usize] = true;
        let mut exits = 0;
        while let Some((x, y)) = queue.pop_front() {
            for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                let (nx, ny) = (x + dx, y + dy);
                if !World::in_map(nx, ny) || seen[(nx * 64 + ny) as usize] {
                    continue;
                }
                if (0x71..=0x73).contains(&w.tile(nx, ny)) {
                    exits += 1;
                }
                if pass(nx, ny) {
                    seen[(nx * 64 + ny) as usize] = true;
                    queue.push_back((nx, ny));
                }
            }
        }
        let total = (0..64)
            .flat_map(|x| (0..64).map(move |y| (x, y)))
            .filter(|&(x, y)| (0x71..=0x73).contains(&w.tile(x, y)))
            .count();
        eprintln!("map {mapon:2}: {exits} exit faces reachable, {total} exit tiles");
        if exits == 0 {
            unreachable.push(mapon);
        }
    }
    assert!(unreachable.is_empty(), "no reachable exit on maps {unreachable:?}");
}
