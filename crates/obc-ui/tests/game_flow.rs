//! Data-gated end-to-end flows through the real screens: level completion,
//! save and load, death and the high-score entry (GAMEFLOW §5, §8, §9).

use obc_audio::Mixer;
use obc_core::input::{InputState, sc};
use obc_formats::DataDir;
use obc_sim::world::ex;
use obc_ui::assets::Assets;
use obc_ui::game::Game;
use obc_ui::screen::{Ctx, Requests, Screen, Transition};
use obc_ui::settings::Settings;

struct Harness {
    assets: Assets,
    settings: Settings,
    saves: std::path::PathBuf,
    stack: Vec<Box<dyn Screen>>,
    audio: obc_audio::MixerHandle,
    _mixer: Mixer,
}

impl Harness {
    fn new(tag: &str) -> Option<Self> {
        let dir = obc_testdata::data_dir()?;
        let assets = Assets::load(DataDir::open(dir).ok()?).ok()?;
        let (mixer, audio) = Mixer::new(48_000);
        let saves =
            std::env::temp_dir().join(format!("obcr-test-saves-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&saves);
        Some(Self {
            assets,
            settings: Settings::default(),
            saves,
            stack: Vec::new(),
            audio,
            _mixer: mixer,
        })
    }

    /// One tic of the top screen with `input`, applying its transition.
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

    fn idle(&mut self, tics: u32) {
        let input = InputState::default();
        for _ in 0..tics {
            self.tick(&input);
        }
    }

    fn top(&self) -> &'static str {
        self.stack.last().map_or("none", |s| s.name())
    }

    fn game(&mut self) -> &mut Game {
        self.stack.iter_mut().rev().find_map(|s| s.game_mut()).expect("no game on the stack")
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
fn completing_a_floor_autosaves_plays_the_sequence_and_starts_the_next() {
    let Some(mut h) = Harness::new("floor") else {
        eprintln!("skipped: OBC_DATA_DIR not set");
        return;
    };
    let g = h.with_ctx(|ctx| Game::new(ctx, 1));
    h.stack.push(Box::new(g));
    h.idle(100);
    let health = h.game().world().gamestate.health;
    h.game().world_mut().playstate = ex::COMPLETED;
    let mut saw_sequence = false;
    for _ in 0..2000 {
        h.idle(1);
        saw_sequence |= h.top() == "sequence";
        if h.top() == "game" && h.game().world().gamestate.mapon == 1 {
            break;
        }
    }
    assert!(saw_sequence, "F2 should play after a sewer floor");
    let w = h.game().world();
    assert_eq!(w.gamestate.mapon, 1);
    // +35 on the floor change, then the new level's start-health cap (400).
    assert_eq!(w.gamestate.health, (health + 35).min(400));
    let slots = obc_ui::saves::list(&h.saves);
    let auto = slots[obc_ui::saves::AUTOSAVE].as_ref().expect("AutoSave written");
    assert_eq!(auto.mapon, 0, "AutoSave holds the completed floor");
    let _ = std::fs::remove_dir_all(&h.saves);
}

#[test]
fn a_saved_game_loads_and_plays() {
    let Some(mut h) = Harness::new("load") else {
        eprintln!("skipped: OBC_DATA_DIR not set");
        return;
    };
    let g = h.with_ctx(|ctx| Game::new_at(ctx, 2, 6));
    h.stack.push(Box::new(g));
    h.idle(300);
    let before = h.game().world().clone();
    let (saves, assets) = (h.saves.clone(), &h.assets);
    obc_ui::saves::write(&saves, 3, &before, "TEST", assets).unwrap();
    let (header, w) = obc_ui::saves::read(&saves, 3, assets).unwrap();
    assert_eq!((header.name.as_str(), header.mapon), ("TEST", 6));
    let g = h.with_ctx(|ctx| Game::from_world(ctx, w));
    h.stack.clear();
    h.stack.push(Box::new(g));
    h.idle(300);
    let w = h.game().world();
    assert_eq!(w.gamestate.mapon, 6);
    assert_eq!(w.gamestate.score, before.gamestate.score);
    let _ = std::fs::remove_dir_all(&h.saves);
}

#[test]
fn dying_ends_in_the_high_score_entry_then_the_menu() {
    let Some(mut h) = Harness::new("death") else {
        eprintln!("skipped: OBC_DATA_DIR not set");
        return;
    };
    let g = h.with_ctx(|ctx| Game::new(ctx, 1));
    h.stack.push(Box::new(g));
    h.idle(50);
    {
        let w = h.game().world_mut();
        w.gamestate.score = 999_999;
        w.gamestate.health = 0;
        w.playstate = ex::DIED;
    }
    let mut seen = Vec::new();
    for _ in 0..3000 {
        h.idle(1);
        if seen.last() != Some(&h.top()) {
            seen.push(h.top());
        }
        if h.top() == "high-scores" {
            break;
        }
    }
    assert_eq!(seen.last(), Some(&"high-scores"), "screens: {seen:?}");
    // Type a name and confirm.
    let input = InputState { text: "TESTER".into(), ..InputState::default() };
    h.tick(&input);
    let mut input = InputState::default();
    input.key(sc::ENTER, true, false);
    h.tick(&input);
    assert_eq!(h.settings.game.scores[0].name, "TESTER");
    assert_eq!(h.settings.game.scores[0].score, 999_999);
    h.idle(20);
    let mut input = InputState::default();
    input.key(sc::SPACE, true, false);
    h.tick(&input);
    assert_eq!(h.top(), "menu");
}
