//! Data-gated: a mod built here (never stored) plays through the real
//! screens: its floors in order, then the victory path, with its own saves
//! and high scores (docs/MODS_AND_OBCEDIT.md §4).

use obc_audio::Mixer;
use obc_core::input::InputState;
use obc_formats::DataDir;
use obc_formats::maps::{Map, write_archive};
use obc_sim::world::ex;
use obc_ui::assets::Assets;
use obc_ui::game::Game;
use obc_ui::screen::{Ctx, Requests, Screen, Transition};
use obc_ui::settings::Settings;

/// A walled 64×64 room, the player at (2, 2) facing east, a stairs exit in
/// the wall east of him, and `enemies` soldiers along row 10.
fn room(name: &str, enemies: usize) -> Map {
    let n = 64 * 64;
    let mut p0 = vec![144u16; n];
    for i in 0..64 {
        for j in [i, 63 * 64 + i, i * 64, i * 64 + 63] {
            p0[j] = 1;
        }
    }
    p0[2 * 64 + 3] = 114; // stairs, east of the start
    let mut p1 = vec![18u16; n];
    p1[2 * 64 + 2] = 20;
    for e in 0..enemies {
        p1[10 * 64 + 10 + e] = 116; // a soldier standing east
    }
    Map { name: name.into(), width: 64, height: 64, planes: [p0, p1, vec![0; n]] }
}

fn write_mod(dir: &std::path::Path) {
    std::fs::create_dir_all(dir).unwrap();
    let (a, b) = (room("FIRST", 0), room("SECOND", 2));
    let (head, body) = write_archive(&[Some(&a), Some(&b)], 0xABCD);
    std::fs::write(dir.join("MAPHEAD.BC"), head).unwrap();
    std::fs::write(dir.join("MAPTEMP.BC"), body).unwrap();
    std::fs::write(
        dir.join("mod.toml"),
        "format = 1\n[mod]\nid = \"test-two\"\ntitle = \"Two Rooms\"\nversion = \"1\"\n\
         [[level]]\nmap = 0\nlike = 6\nmessage = \"WELCOME\"\nsequence = \"none\"\n\
         [[level]]\nmap = 1\nlike = 2\n",
    )
    .unwrap();
}

struct H {
    assets: Assets,
    settings: Settings,
    saves: std::path::PathBuf,
    stack: Vec<Box<dyn Screen>>,
    audio: obc_audio::MixerHandle,
    _mixer: Mixer,
}

impl H {
    fn tick(&mut self, input: &InputState) {
        let mut requests = Requests::default();
        let mut ctx = Ctx {
            assets: &self.assets,
            audio: &self.audio,
            input,
            settings: &mut self.settings,
            requests: &mut requests,
            save_dir: &self.saves,
        };
        let top = self.stack.last_mut().expect("empty stack");
        match top.tick(&mut ctx) {
            Transition::None => {}
            Transition::Push(s) => self.stack.push(s),
            Transition::Pop => {
                self.stack.pop();
            }
            Transition::Replace(s) => {
                self.stack.pop();
                self.stack.push(s);
            }
            Transition::Reset(s) => {
                self.stack.clear();
                self.stack.push(s);
            }
            Transition::Quit => self.stack.clear(),
        }
    }

    fn top(&self) -> &'static str {
        self.stack.last().map_or("none", |s| s.name())
    }

    fn game(&mut self) -> &mut Game {
        self.stack.iter_mut().rev().find_map(|s| s.game_mut()).expect("no game")
    }

    fn with_ctx<R>(&mut self, f: impl FnOnce(&mut Ctx) -> R) -> R {
        let input = InputState::default();
        let mut requests = Requests::default();
        let mut ctx = Ctx {
            assets: &self.assets,
            audio: &self.audio,
            input: &input,
            settings: &mut self.settings,
            requests: &mut requests,
            save_dir: &self.saves,
        };
        f(&mut ctx)
    }
}

#[test]
fn a_mod_plays_its_floors_then_wins() {
    let Some(dir) = obc_testdata::data_dir() else {
        eprintln!("SKIPPED: original game data not found (set OBC_DATA_DIR)");
        return;
    };
    let root = std::env::temp_dir().join(format!("obcr-mod-test-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let saves = root.join("data").join("saves");
    let mod_dir = root.join("data").join("mods").join("two-rooms");
    write_mod(&mod_dir);

    let assets = Assets::load(DataDir::open(dir).unwrap()).unwrap();
    let (mixer, audio) = Mixer::new(48_000);
    let mut h =
        H { assets, settings: Settings::default(), saves, stack: Vec::new(), audio, _mixer: mixer };

    // Installed, listed by the MODS screen, chosen there.
    let (m, problems) = obc_ui::campaign::check(&mod_dir).unwrap();
    assert!(problems.is_empty(), "{problems:?}");
    assert_eq!(m.manifest.levels.len(), 2);
    h.settings.game.active_mod = "test-two".into();
    assert_eq!(h.with_ctx(|ctx| obc_ui::campaign::active_id(ctx)), "test-two");

    let g = h.with_ctx(|ctx| Game::new(ctx, 1));
    h.stack.push(Box::new(g));
    h.tick(&InputState::default());
    {
        let w = h.game().world_mut();
        assert_eq!(w.gamestate.mapon, 0);
        assert_eq!(w.level.message.as_deref(), Some("WELCOME"));
        assert!(!w.level.dark, "like = 6: a lit UN floor");
    }

    // A save lands in the mod's own folder and says so.
    let snapshot = h.game().world().clone();
    h.with_ctx(|ctx| obc_ui::saves::write(ctx, 1, &snapshot, "MODSAVE")).unwrap();
    assert!(h.saves.join("test-two").join("svgm1.obcr").is_file());
    h.settings.game.active_mod.clear();
    assert!(h.with_ctx(|ctx| obc_ui::saves::read(ctx, 1)).is_err(), "not in the original's slots");
    h.settings.game.active_mod = "test-two".into();
    assert!(h.with_ctx(|ctx| obc_ui::saves::read(ctx, 1)).is_ok());

    // Finish floor 1 (no enemies; no sequence): floor 2, a dark sewer-like room.
    h.game().world_mut().playstate = ex::COMPLETED;
    for _ in 0..600 {
        h.tick(&InputState::default());
        if h.top() == "game" && h.game().world().gamestate.mapon == 1 {
            break;
        }
    }
    let w = h.game().world();
    assert_eq!(w.gamestate.mapon, 1);
    assert!(w.level.dark && w.level.sewer, "like = 2");
    assert_eq!(w.gamestate.killtotal, 2);

    // Finishing the last floor wins the campaign: victory picture, then scores.
    h.game().world_mut().playstate = ex::COMPLETED;
    let mut seen = Vec::new();
    for _ in 0..4000 {
        h.tick(&InputState::default());
        if seen.last() != Some(&h.top()) {
            seen.push(h.top());
        }
        if h.top() == "high-scores" {
            break;
        }
        if h.top() == "victory" {
            let mut i = InputState::default();
            i.key(obc_core::input::sc::SPACE, true, false);
            h.tick(&i);
        }
    }
    assert!(seen.contains(&"victory"), "screens: {seen:?}");
    assert_eq!(seen.last(), Some(&"high-scores"), "screens: {seen:?}");
    assert!(h.settings.game.mod_scores.contains_key("test-two"), "the mod's own table");
    let _ = std::fs::remove_dir_all(&root);
}
