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
    let mut c = TicCmd::default();
    c.controly = -70;
    if stuck > 3 || frame % 97 < 12 {
        c.controlx = if (frame / 200) % 2 == 0 { 70 } else { -70 };
    }
    c.buttons[bt::ATTACK] = frame % 40 < 20;
    c.buttons[bt::USE] = frame % 7 == 0;
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
