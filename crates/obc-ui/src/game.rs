//! The in-game screen (`GameLoop` 1823:0ea3, `PlayLoop` 1942:1315).
//!
//! Owns the [`World`] and runs one original frame of two tics every other
//! 70 Hz tic (ADR-013). Simulation events drive sound, the status bar,
//! palette flashes, the tremor shake and gamepad rumble. Level ends follow
//! GAMEFLOW §5.3–5.5: exit sequence and next map, death sequence and the
//! menu, or victory.

use std::sync::Arc;

use obc_audio::AudioCommand;
use obc_render::Framebuffer;
use obc_render::palette::{Palette, expand6, faded};
use obc_render::view3d::{Scene, SplatView, View3D, build_shade_table};
use obc_sim::events::SimEvent;
use obc_sim::world::{TicCmd, World, ex};

use crate::assets::Assets;
use crate::controls::Controls;
use crate::hud::Hud;
use crate::scene::{SceneBuffers, floor_params, weapon_view};
use crate::screen::{Ctx, Frame, Screen, Transition};
use crate::sequence::Sequence;
use crate::sound::{Listener, SoundManager};
use crate::viewer::{Layout, layout, tile_background};

/// Tics per original frame (ADR-013).
const FRAME_TICS: i16 = 2;
/// Red and white palette shifts (RENDERER.md §8.1).
const NUMREDSHIFTS: i32 = 4;
const REDSTEPS: i32 = 8;
const NUMWHITESHIFTS: i32 = 3;
const WHITESTEPS: i32 = 10;
const WHITETICS: i32 = 6;
/// `T2[c]` of the page-flip shake (DS:0x0D51): two lines up on odd counts.
const SHAKE_LINES: [usize; 2] = [0, 2];
/// The low-health blackout: 50 fade steps, one per tic.
const BLACKOUT: u32 = 50;
/// Level fade-in and exit fade-out, in tics.
const FADE_IN: u32 = 30;
const FADE_OUT: u32 = 30;
/// `Died`: scream wait cap and the two fades (dark red, then black).
const SCREAM_WAIT: u32 = 49;
const DEATH_RED: u32 = 150;
const DEATH_BLACK: u32 = 50;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum After {
    NextLevel,
    /// Death or victory: the high-score check, then the menu.
    Scores,
    /// After the victory picture: F7, then the scores.
    Victory,
    Menu,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    FadeIn(u32),
    Playing,
    /// Turning to face the killer, then waiting for the scream.
    Dying {
        target: Option<i16>,
        wait: u32,
    },
    DeathFade(u32),
    ExitFade(u32),
    /// A sequence was pushed; act when it pops.
    Sequence(After),
    /// Victory: hold the view for up to 1000 tics or a key.
    Victory(u32),
}

pub struct Game {
    world: World,
    sound: SoundManager,
    hud: Hud,
    controls: Controls,
    view: Option<(Layout, View3D)>,
    buffers: SceneBuffers,
    splats: Vec<SplatView>,
    shade: [[u8; 256]; 2],
    phase: Phase,
    odd_tic: bool,
    damagecount: i32,
    bonuscount: i32,
    /// Palette mode: 1 normal, 2 dark (generators off).
    lighting: u8,
    shake_lines: usize,
    floor_mode: u8,
    show_health: bool,
    crosshair: bool,
    paused: bool,
    /// The control panel was opened; restart the music on return.
    in_menu: bool,
    sparkle_frames: u32,
    /// `viewsize` (12–20).
    viewsize: u16,
    /// `MusicMode` (F2): music on.
    music: bool,
    /// Overhead-map player blink (DS:0x3272), toggled each drawn frame.
    map_blink: bool,
    /// The bogus-mode warp prompt being typed, and the warp it produced.
    warp_prompt: Option<String>,
    pending_cheat: obc_sim::world::Cheat,
    /// Tics left of a low-health blackout (`VL_FadeOut(…, 50)` in TakeDamage
    /// 208c:05b5); the game is frozen while it runs, as the original blocks.
    blackout: u32,
    /// F10 `CP_CheckQuick`: `BACK TO DOS` asked over the game.
    quit_prompt: bool,
    /// Enhanced interpolation: player pose and actor positions at the last
    /// frame boundary, and whether it is enabled.
    prev_pose: (i32, i32, i16),
    prev_objs: Vec<Option<(i32, i32)>>,
    interpolate: bool,
}

impl Game {
    /// `NewGame(difficulty)` and the first level.
    pub fn new(ctx: &mut Ctx, difficulty: i16) -> Self {
        Self::new_at(ctx, difficulty, 0)
    }

    /// A new game starting at `mapon` (developer warp; the original's warp
    /// cheat keeps the player state instead).
    pub fn new_at(ctx: &mut Ctx, difficulty: i16, mapon: i16) -> Self {
        let a = ctx.assets;
        let mut world = World::new(a.game.clone(), a.trig.clone());
        obc_sim::player::new_game(&mut world, difficulty);
        world.gamestate.mapon = mapon;
        let mut g = Self {
            world,
            sound: SoundManager::new(a, ctx.settings.audio.channels),
            hud: Hud::default(),
            controls: Controls::default(),
            view: None,
            buffers: SceneBuffers::default(),
            splats: Vec::new(),
            shade: [build_shade_table(false), build_shade_table(true)],
            phase: Phase::FadeIn(0),
            odd_tic: false,
            damagecount: 0,
            bonuscount: 0,
            lighting: 1,
            shake_lines: 0,
            floor_mode: 2,
            show_health: false,
            crosshair: false,
            paused: false,
            in_menu: false,
            sparkle_frames: 0,
            viewsize: 18,
            music: true,
            map_blink: false,
            warp_prompt: None,
            pending_cheat: obc_sim::world::Cheat::None,
            blackout: 0,
            quit_prompt: false,
            prev_pose: (0, 0, 0),
            prev_objs: Vec::new(),
            interpolate: false,
        };
        g.start_level(ctx);
        g
    }

    /// A loaded game: `GameLoop` skips `SetupGameLevel` (`loadedgame`), then
    /// `StartMusic` and the `PlayLoop` entry as usual.
    pub fn from_world(ctx: &mut Ctx, mut world: World) -> Self {
        world.loadedgame = false;
        let mut g = Self::new_at(ctx, world.gamestate.difficulty, 0);
        g.world = world;
        g.set_view_size(ctx.settings.game.viewsize);
        g.world.events.clear();
        g.start_music(ctx);
        g.world.start_play_loop();
        g.hud = Hud::default();
        g.capture_prev();
        g.phase = Phase::FadeIn(0);
        g.drain_events(ctx);
        g
    }

    pub fn world(&self) -> &World {
        &self.world
    }

    /// For tools and tests that steer a game.
    pub fn world_mut(&mut self) -> &mut World {
        &mut self.world
    }

    /// F9 `QuickSave` (1618:1210): the slot last saved from the menu.
    fn quick_save(&mut self, ctx: &mut Ctx) {
        let Some(slot) = ctx.settings.game.quicksave_slot.map(usize::from) else {
            self.hud.message("NO CURRENT SAVE".into());
            return;
        };
        if slot >= crate::saves::SLOTS {
            self.hud.message("UNABLE TO SAVE".into());
            return;
        }
        let name = crate::saves::list(ctx.save_dir)[slot]
            .as_ref()
            .map_or_else(|| format!("SLOT {}", slot + 1), |h| h.name.clone());
        match crate::saves::write(ctx.save_dir, slot, &self.world, &name, ctx.assets) {
            Ok(()) => self.hud.message(format!("SAVED {name}")),
            Err(e) => {
                eprintln!("quicksave: {e}");
                self.hud.message(format!("CANT SAVE {name}"));
            }
        }
    }

    /// `SetupGameLevel`, `StartMusic`, then the `PlayLoop` entry.
    fn start_level(&mut self, ctx: &mut Ctx) {
        let mapon = self.world.gamestate.mapon;
        let Ok(Some(map)) = ctx.assets.maps.map(mapon.max(0) as usize) else {
            eprintln!("map {mapon} missing; returning to the menu");
            self.phase = Phase::Sequence(After::Menu);
            return;
        };
        self.set_view_size(ctx.settings.game.viewsize);
        self.world.setup_game_level(&map, mapon);
        self.start_music(ctx);
        self.lighting = self.world.start_play_loop();
        self.hud.event = Default::default();
        self.capture_prev();
        self.damagecount = 0;
        self.bonuscount = 0;
        self.phase = Phase::FadeIn(0);
        self.drain_events(ctx);
    }

    /// `NewViewSize`: the view geometry used by the gameplay visibility pass,
    /// `GunAttack` and the splats.
    fn set_view_size(&mut self, size: u16) {
        let v = size.clamp(12, 20) & !1;
        self.viewsize = v;
        self.world.view = obc_sim::vis::ViewGeom::new(v);
        let v = v as i16;
        self.world.ply.set_view_size(v * 16, v * 8);
    }

    /// `StartMusic` (1942:0d2d): one `US_RndT` on maps ≥ 10.
    fn start_music(&mut self, ctx: &Ctx) {
        let song = self.world.level_song();
        if !self.music {
            return;
        }
        match ctx.assets.audio.music(300 + usize::from(song)) {
            Ok(m) => ctx.audio.send(AudioCommand::StartMusic(Arc::from(m))),
            Err(e) => eprintln!("music {song}: {e}"),
        }
    }

    /// Records the pose and actor positions at a frame boundary.
    fn capture_prev(&mut self) {
        let p = self.world.plr();
        self.prev_pose = (p.x, p.y, p.angle);
        self.prev_objs.clear();
        self.prev_objs.extend(self.world.objs.iter().map(|o| (o.state != 0).then_some((o.x, o.y))));
    }

    fn listener(&self) -> Listener {
        let p = self.world.plr();
        Listener { x: p.x, y: p.y, angle: p.angle }
    }

    fn drain_events(&mut self, ctx: &mut Ctx) {
        let l = self.listener();
        for e in std::mem::take(&mut self.world.events) {
            match e {
                SimEvent::Sound(id) => {
                    self.sound.play(ctx.audio, id, None, l);
                    if (45..=49).contains(&id) {
                        ctx.requests.rumble = Some((0.15, 0.3, 60));
                    }
                }
                SimEvent::SoundAt { id, x, y } => self.sound.play(ctx.audio, id, Some((x, y)), l),
                SimEvent::DamageFlash(n) => {
                    self.damagecount += i32::from(n);
                    let s = (f32::from(n) / 40.0).clamp(0.2, 1.0);
                    ctx.requests.rumble = Some((s, s * 0.6, 180));
                }
                SimEvent::BonusFlash => self.bonuscount = NUMWHITESHIFTS * WHITETICS,
                SimEvent::Message(m) => self.hud.message(m),
                SimEvent::ItemIcon(i) => self.hud.show_item(i),
                SimEvent::AllyDied => self.hud.ally_died(),
                SimEvent::Shake(n) => {
                    let s = (f32::from(n) / 8.0).clamp(0.3, 1.0);
                    ctx.requests.rumble = Some((s, s, 400));
                }
                SimEvent::Rumble(n) => {
                    let s = f32::from(n) / 100.0;
                    ctx.requests.rumble = Some((s, s, 250));
                }
                SimEvent::Lighting(n) => self.lighting = n,
                SimEvent::Fault(f) => eprintln!("sim fault: {f}"),
                SimEvent::Blackout => self.blackout = BLACKOUT,
                SimEvent::Splat(_) | SimEvent::Hud => {}
            }
        }
    }

    /// One original frame of play.
    fn play_frame(&mut self, ctx: &mut Ctx, cmd: &TicCmd) {
        let was_full = self.world.objlistfull;
        self.world.frame(cmd, FRAME_TICS);
        self.drain_events(ctx);
        self.sound.update(ctx.audio, self.listener());
        self.hud.frame(FRAME_TICS);
        self.palette_shifts(FRAME_TICS);
        if self.world.ply.map_overlay == 1 {
            self.map_blink = !self.map_blink;
        }
        self.sparkle_frames += 1;
        if self.sparkle_frames > 300 || (self.world.objlistfull && !was_full) {
            self.sparkle_frames = 0;
            self.hud.sparkle();
        }
        // Page flip with shake (1b4a:1420): only with TREMORS on and health ≥ 20.
        self.shake_lines = 0;
        if self.world.shake > 0 {
            if self.world.ply.tremors && self.world.gamestate.health >= 20 {
                self.shake_lines = SHAKE_LINES[(self.world.shake & 1) as usize];
            }
            self.world.shake -= 1;
        }
    }

    /// `UpdatePaletteShifts`: counters only; the palette is built when drawing.
    fn palette_shifts(&mut self, tics: i16) {
        let t = i32::from(tics);
        self.bonuscount = (self.bonuscount - t).max(0);
        self.damagecount = (self.damagecount - t).max(0);
    }

    fn base_palette(&self, a: &Assets) -> Palette {
        if self.lighting == 2 { a.darkpal } else { a.gamepal }
    }

    /// The palette for this frame: red or white shift (indices 0–15 untinted).
    fn view_palette(&self, a: &Assets) -> Palette {
        let base = self.base_palette(a);
        let red =
            if self.damagecount > 0 { (self.damagecount / 10 + 1).min(NUMREDSHIFTS) } else { 0 };
        let white = if self.bonuscount > 0 {
            (self.bonuscount / WHITETICS + 1).min(NUMWHITESHIFTS)
        } else {
            0
        };
        let (i, steps, target) = if red > 0 {
            (red, REDSTEPS, [64, 0, 0])
        } else if white > 0 {
            (white, WHITESTEPS, [64, 62, 0])
        } else {
            return base;
        };
        let mut out = base;
        for (c, o) in base.iter().zip(out.iter_mut()).skip(16) {
            for ch in 0..3 {
                let v = i32::from(c[ch] >> 2);
                let v = v + (target[ch] - v) * i / steps;
                o[ch] = expand6(v.clamp(0, 63) as u8);
            }
        }
        out
    }

    fn level_ended(&mut self, ctx: &mut Ctx) {
        match self.world.playstate {
            ex::DIED => {
                let target = self.world.start_death();
                self.drain_events(ctx);
                self.phase = Phase::Dying { target, wait: 0 };
            }
            ex::VICTORIOUS => {
                self.world.shake = 0;
                self.world.quake = 0;
                self.phase = Phase::Victory(0);
            }
            ex::WARPED => {
                // The warp keeps the player's state (GAMEFLOW §5.6); no sequence.
                ctx.audio.send(AudioCommand::StopMusic);
                self.start_level(ctx);
            }
            ex::COMPLETED => {
                ctx.audio.send(AudioCommand::StopMusic);
                // AutoSave before `mapon` advances (GAMEFLOW §9.3).
                let w = &self.world;
                if w.gamestate.difficulty < 4
                    && let Err(e) = crate::saves::write(
                        ctx.save_dir,
                        crate::saves::AUTOSAVE,
                        w,
                        "AutoSave",
                        ctx.assets,
                    )
                {
                    eprintln!("autosave: {e}");
                }
                self.phase = Phase::ExitFade(0);
            }
            _ => {
                ctx.audio.send(AudioCommand::StopMusic);
                self.phase = Phase::Sequence(After::Menu);
            }
        }
    }

    /// The exit sequence for the level just finished (GAMEFLOW §5.3).
    fn exit_sequence(&self) -> &'static str {
        match self.world.gamestate.mapon {
            0..=4 => "F2",
            22 => "F5",
            _ if self.world.ply.exitkind == 1 => "F4",
            _ => "F3",
        }
    }

    fn push_sequence(&mut self, ctx: &Ctx, name: &str, after: After) -> Transition {
        self.sound.stop_all(ctx.audio);
        self.phase = Phase::Sequence(after);
        match Sequence::load(ctx.assets, name) {
            Some(s) => Transition::Push(Box::new(s)),
            None => Transition::None,
        }
    }
}

impl Screen for Game {
    fn tick(&mut self, ctx: &mut Ctx) -> Transition {
        ctx.requests.relative_mouse = Some(true);
        self.sound.advance(1);
        if self.in_menu {
            // Back from the control panel: StartMusic again (GAMEFLOW §11.3).
            self.in_menu = false;
            self.set_view_size(ctx.settings.game.viewsize);
            self.sound.set_channels(ctx.settings.audio.channels);
            self.start_music(ctx);
        }
        if self.blackout > 0 {
            self.blackout -= 1;
            return Transition::None;
        }
        if self.quit_prompt {
            let input = ctx.input;
            if input.pressed(obc_core::input::sc::Y)
                || input.pad_pressed(obc_core::input::pad::SOUTH)
            {
                ctx.audio.send(AudioCommand::StopMusic);
                self.sound.stop_all(ctx.audio);
                return Transition::Reset(Box::new(crate::menu::QuitScreen::default()));
            }
            if input.pressed(obc_core::input::sc::N)
                || input.pressed(obc_core::input::sc::ESCAPE)
                || input.pad_pressed(obc_core::input::pad::EAST)
            {
                self.quit_prompt = false;
            }
            return Transition::None;
        }
        if let Some(text) = &mut self.warp_prompt {
            // ` FLOOR: ` (1942:0b4a): two digits, Enter to warp, Esc to cancel.
            let input = ctx.input;
            for c in input.text.chars().filter(char::is_ascii_digit) {
                if text.len() < 2 {
                    text.push(c);
                }
            }
            if input.pressed(obc_core::input::sc::BACKSPACE) {
                text.pop();
            }
            if input.pressed(obc_core::input::sc::ENTER) {
                let n = text.parse::<u8>().unwrap_or(0);
                self.warp_prompt = None;
                ctx.requests.text_input = Some(false);
                self.pending_cheat = obc_sim::world::Cheat::Warp(n);
            } else if input.pressed(obc_core::input::sc::ESCAPE) {
                self.warp_prompt = None;
                ctx.requests.text_input = Some(false);
            }
            return Transition::None;
        }
        self.controls.accumulate(ctx.input, &ctx.settings.input, ctx.settings.game.nsr250);
        self.odd_tic = !self.odd_tic;
        if self.odd_tic {
            return Transition::None;
        }
        let (mut cmd, ui) =
            self.controls.build(ctx.input, &ctx.settings.input, &self.world, FRAME_TICS);
        if self.pending_cheat != obc_sim::world::Cheat::None {
            cmd.cheat = std::mem::take(&mut self.pending_cheat);
        }
        self.interpolate = ctx.settings.video.interpolate;
        self.capture_prev();

        match self.phase {
            Phase::FadeIn(t) => {
                self.phase = if t + 2 >= FADE_IN { Phase::Playing } else { Phase::FadeIn(t + 2) };
                Transition::None
            }
            Phase::Playing => {
                if self.paused {
                    if ui.pause || ui.menu || cmd.buttons.iter().any(|&b| b) {
                        self.paused = false;
                        ctx.audio.send(AudioCommand::PauseMusic(false));
                    }
                    return Transition::None;
                }
                // Esc is ignored while a tremor shakes the screen with TREMORS on
                // (DS:0x0D40 and DS:0x0D44, CheckKeys).
                if ui.menu && !(self.world.shake > 0 && self.world.ply.tremors) {
                    ctx.audio.send(AudioCommand::StopMusic);
                    self.sound.stop_all(ctx.audio);
                    self.in_menu = true;
                    return Transition::Push(crate::menu::in_game(Box::new(self.world.clone())));
                }
                if ui.quit {
                    self.quit_prompt = true;
                    return Transition::None;
                }
                if ui.pause {
                    self.paused = true;
                    ctx.audio.send(AudioCommand::PauseMusic(true));
                    return Transition::None;
                }
                // F1 / F2 (US_ControlPanel 1618:061a): sound and music toggles.
                if ui.sound_toggle {
                    self.sound.enabled = !self.sound.enabled;
                    if !self.sound.enabled {
                        self.sound.stop_all(ctx.audio);
                    }
                    let m = if self.sound.enabled { "SOUND  ON" } else { "SOUND  OFF" };
                    self.hud.message(m.into());
                }
                if ui.music_toggle {
                    self.music = !self.music;
                    if self.music {
                        // SD_MusicOn resumes the current song.
                        ctx.audio.send(AudioCommand::PauseMusic(false));
                    } else {
                        ctx.audio.send(AudioCommand::PauseMusic(true));
                    }
                    self.hud.message(if self.music { "MUSIC ON" } else { "MUSIC OFF" }.into());
                }
                if ui.warp && self.world.gamestate.cheatmode {
                    self.warp_prompt = Some(String::new());
                    ctx.requests.text_input = Some(true);
                    return Transition::None;
                }
                if ui.quicksave && self.world.gamestate.difficulty <= 4 {
                    self.quick_save(ctx);
                }
                // F5 / F6 (1942:07a5, 07b2): textured floors and ceilings off / on
                // (`floormode` DS:0x33D2 = 0 / 2, kept in CONFIG.DAT).
                if ui.floor_off {
                    ctx.settings.game.floor_textures = false;
                }
                if ui.floor_on {
                    ctx.settings.game.floor_textures = true;
                }
                self.floor_mode = if ctx.settings.game.floor_textures { 2 } else { 0 };
                self.show_health ^= ui.health_overlay;
                self.crosshair ^= ui.crosshair;
                self.play_frame(ctx, &cmd);
                if self.world.level_done() {
                    self.level_ended(ctx);
                }
                Transition::None
            }
            Phase::Dying { target, wait } => {
                self.palette_shifts(FRAME_TICS);
                if let Some(t) = target
                    && !self.world.death_turn(t, FRAME_TICS)
                {
                    return Transition::None;
                }
                let wait = wait + FRAME_TICS as u32;
                self.phase = if wait >= SCREAM_WAIT || !self.sound.digi_playing() {
                    // FinishPaletteShifts, lighting mode 1.
                    self.damagecount = 0;
                    self.bonuscount = 0;
                    self.lighting = 1;
                    ctx.audio.send(AudioCommand::StopMusic);
                    Phase::DeathFade(0)
                } else {
                    Phase::Dying { target: None, wait }
                };
                Transition::None
            }
            Phase::DeathFade(t) => {
                let t = t + FRAME_TICS as u32;
                if t >= DEATH_RED + DEATH_BLACK {
                    return self.push_sequence(ctx, "F6", After::Scores);
                }
                self.phase = Phase::DeathFade(t);
                Transition::None
            }
            Phase::ExitFade(t) => {
                let t = t + FRAME_TICS as u32;
                if t >= FADE_OUT {
                    let name = self.exit_sequence();
                    return self.push_sequence(ctx, name, After::NextLevel);
                }
                self.phase = Phase::ExitFade(t);
                Transition::None
            }
            Phase::Sequence(After::NextLevel) => {
                // ClearFloorItems, mapon++, health += 35.
                self.world.advance_level();
                self.start_level(ctx);
                Transition::None
            }
            Phase::Sequence(After::Menu) => Transition::Reset(crate::menu::main_menu(ctx.assets)),
            Phase::Sequence(After::Victory) => self.push_sequence(ctx, "F7", After::Scores),
            Phase::Sequence(After::Scores) => {
                // DrawHighScores(1): name entry if the score qualifies (not in bogus mode).
                let g = &self.world.gamestate;
                let entry = (!g.cheatmode).then_some((g.score, (g.mapon + 1) as u16));
                Transition::Reset(Box::new(crate::menu::HighScores::new(ctx, entry)))
            }
            Phase::Victory(t) => {
                let t = t + FRAME_TICS as u32;
                if t >= 1000 || ui.menu || cmd.buttons.iter().any(|&b| b) {
                    ctx.audio.send(AudioCommand::StopMusic);
                    // VictoryScreen, then F7 (not shipped), then the scores (GAMEFLOW §5.5).
                    self.sound.stop_all(ctx.audio);
                    self.phase = Phase::Sequence(After::Victory);
                    return Transition::Push(Box::new(crate::menu::VictoryPic::default()));
                }
                self.phase = Phase::Victory(t);
                Transition::None
            }
        }
    }

    fn game_mut(&mut self) -> Option<&mut Game> {
        Some(self)
    }

    fn name(&self) -> &'static str {
        "game"
    }

    fn draw(&mut self, a: &Assets, out: &mut Frame) {
        let p = out.presentation;
        let l = layout(p, self.viewsize);
        if self.view.as_ref().is_none_or(|(vl, _)| *vl != l) {
            self.view = Some((l, View3D::new(l.cfg)));
        }
        tile_background(out.frame, a, p.scale);
        let w = &self.world;
        // Time since the last frame boundary, in frames (Enhanced only).
        let lerp = (self.interpolate && !p.is_classic())
            .then(|| ((f32::from(u8::from(self.odd_tic)) + out.alpha) / 2.0).clamp(0.0, 1.0));
        self.buffers.fill(w, lerp.map(|t| (self.prev_objs.as_slice(), t)));
        let plr = w.plr();
        let (px, py, fine) = match lerp {
            Some(t) => {
                let (x0, y0, a0) = self.prev_pose;
                let blend = |a: i32, b: i32| a + ((b - a) as f32 * t) as i32;
                let da = (i32::from(plr.angle) - i32::from(a0) + 540).rem_euclid(360) - 180;
                let fine = (i32::from(a0) * 10 + (da as f32 * 10.0 * t) as i32).rem_euclid(3600);
                (blend(x0, plr.x), blend(y0, plr.y), Some(fine))
            }
            None => (plr.x, plr.y, None),
        };
        let scene = Scene {
            tiles: &w.tilemap,
            info: &w.plane1,
            plane0: &w.plane0,
            doors: &self.buffers.doors,
            pwall_pos: w.pwall.pos,
            floor: floor_params(&a.game.levels, w, self.floor_mode),
            shade_table: &self.shade[usize::from(w.dark)],
            shade_bias: 2 * i32::from(w.ply.lightbits),
            player_x: px,
            player_y: py,
            player_angle: fine.map_or(i32::from(plr.angle), |f| f / 10),
            player_fine_angle: fine,
            sprites: &self.buffers.sprites,
            weapon: weapon_view(w),
        };
        self.splats.clear();
        for s in &w.ply.splats[..usize::from(w.ply.splatcount)] {
            let (shape, height) = match s.kind {
                2 => (640 + s.shape as u16, i32::from(s.y) >> 1),
                1 => (648 + s.shape as u16, i32::from(s.y)),
                _ => continue,
            };
            self.splats.push(SplatView { shape, x: i32::from(s.x), height });
        }
        if let Some((_, view)) = &mut self.view {
            view.render(out.frame, l.x, l.y, &scene, a);
            view.draw_splats(out.frame, l.x, l.y, &self.splats, a);
        }
        out.layer.clear(0xFF);
        out.layer_key = Some(0xFF);
        let (vx, vy) = (i32::from(160 - self.viewsize * 8), i32::from(80 - self.viewsize * 4));
        let (vw, vh) = (i32::from(self.viewsize) * 16, i32::from(self.viewsize) * 8);
        if self.crosshair {
            draw_crosshair(out.layer, vx + vw / 2, vy + vh / 2);
        }
        if self.show_health {
            draw_health(out.layer, a, w.gamestate.health, vx + vw, vy);
        }
        if w.ply.map_overlay == 1 {
            draw_overhead_map(out.layer, w, self.viewsize, self.map_blink);
        }
        self.hud.draw(out.layer, a, w);
        self.hud.draw_message(out.layer, a, vx, vy);
        if self.paused {
            draw_box(out.layer, a, "PAUSE");
        }
        if let Some(t) = &self.warp_prompt {
            draw_box(out.layer, a, &format!(" FLOOR: {t}_"));
        }
        if self.quit_prompt {
            draw_box(out.layer, a, "BACK TO DOS Y/N");
        }

        let mut pal = self.view_palette(a);
        if self.blackout > 0 {
            pal = faded(&pal, (BLACKOUT - self.blackout) as i32, BLACKOUT as i32, [0, 0, 0]);
        }
        out.palette = match self.phase {
            Phase::FadeIn(t) => faded(&pal, (FADE_IN - t) as i32, FADE_IN as i32, [0, 0, 0]),
            Phase::ExitFade(t) => faded(&pal, t as i32, FADE_OUT as i32, [0, 0, 0]),
            Phase::DeathFade(t) if t < DEATH_RED => {
                // VL_FadeOut to (10, 0, 0) on the 6-bit scale.
                faded(&pal, t as i32, DEATH_RED as i32, [40, 0, 0])
            }
            Phase::DeathFade(t) => {
                let red = faded(&pal, 1, 1, [40, 0, 0]);
                faded(&red, (t - DEATH_RED) as i32, DEATH_BLACK as i32, [0, 0, 0])
            }
            Phase::Sequence(_) => faded(&pal, 1, 1, [0, 0, 0]),
            _ => pal,
        };
        out.scroll = self.shake_lines;
    }
}

/// Starts a game on its first tic (for `--play`, which has no `Ctx` at startup).
pub struct Boot {
    pub difficulty: i16,
    pub mapon: i16,
}

impl Screen for Boot {
    fn tick(&mut self, ctx: &mut Ctx) -> Transition {
        Transition::Replace(Box::new(Game::new_at(ctx, self.difficulty, self.mapon)))
    }

    fn draw(&mut self, _a: &Assets, out: &mut Frame) {
        out.frame.clear(0);
        out.layer.clear(0);
    }
}

/// The Tab overhead map (`2f4d:0d31`, RENDERER.md §9.2), one pixel per tile
/// at absolute page coordinates, drawn over the view without a background.
fn draw_overhead_map(fb: &mut Framebuffer, w: &World, viewsize: u16, blink: bool) {
    let (vw, vh) = (i32::from(viewsize) * 16, i32::from(viewsize) * 8);
    let (x0, y0) = ((320 - vw) / 2 + 4, (200 - vh) / 2 + 4);
    // Frame: a 64×64 outline in 0x0C.
    fb.bar(x0, y0, 64, 1, 0x0C);
    fb.bar(x0, y0 + 63, 64, 1, 0x0C);
    fb.bar(x0, y0, 1, 64, 0x0C);
    fb.bar(x0 + 63, y0, 1, 64, 0x0C);
    // Walls: a 24×24 window around the player, columns from a multiple of 8.
    let p = w.plr();
    let window = |t: u16| -> i32 {
        let c = (i32::from(t as i16) - 12).max(0);
        if c + 24 > 64 { 40 } else { c }
    };
    let (c0, r0) = (window(p.tilex) & !7, window(p.tiley));
    for ty in r0..r0 + 24 {
        let row = w.automap.get(ty as usize).copied().unwrap_or(0);
        for tx in c0..(c0 + 24).min(64) {
            if row & (1 << tx) != 0 {
                fb.put(x0 + tx, y0 + ty, 0x0C);
            }
        }
    }
    if w.gamestate.difficulty < 4 {
        // ThermoSensor: every actor that ran this frame, inside the window.
        if w.gamestate.cheatmode || w.thermosensor {
            for &v in &w.actorpos {
                let (tx, ty) = (i32::from(v >> 8), i32::from(v & 0xFF));
                if v != 0 && (c0..c0 + 24).contains(&tx) && (r0..r0 + 24).contains(&ty) {
                    fb.put(x0 + tx, y0 + ty, 1);
                }
            }
        }
        // Allies, anywhere in the box.
        for id in w.obj_ids() {
            let o = w.obj(id);
            if id != w.player && o.obclass == obc_sim::world::class::ALLY {
                fb.put(x0 + i32::from(o.tilex as i16), y0 + i32::from(o.tiley as i16), 2);
            }
        }
    }
    let c = if blink { 0xFC } else { 0x0F };
    fb.put(x0 + i32::from(p.tilex as i16), y0 + i32::from(p.tiley as i16), c);
}

/// The C crosshair (30d6:07d0): eleven dots every other pixel across and down
/// through the view centre (`viewwidth/2`, `viewheight/2`), colour 0xE1.
fn draw_crosshair(fb: &mut Framebuffer, cx: i32, cy: i32) {
    for i in (0..22).step_by(2) {
        fb.put(cx + i - 10, cy, 0xE1);
        fb.put(cx, cy + i - 10, 0xE1);
    }
}

/// The H health readout (30d6:0779; positions from `SetViewSize` 1497:1143):
/// `%3d` in font 1 at (viewwidth − 27, 0) with a black shadow, in the font's
/// own colours, or flat 0xFE below 50; the health icon (pic 24) at
/// (viewwidth − 40, 2). `right`/`top` are the view's right edge and top.
fn draw_health(fb: &mut Framebuffer, a: &Assets, health: i16, right: i32, top: i32) {
    use obc_render::draw2d::{draw_text, draw_text_native};
    if let Some(font) = a.fonts.get(1) {
        let text = format!("{health:3}");
        let (x, y) = (right - 27, top);
        draw_text(fb, font, text.as_bytes(), x + 1, y + 1, 0);
        if health < 50 {
            draw_text(fb, font, text.as_bytes(), x, y, 0xFE);
        } else {
            draw_text_native(fb, font, text.as_bytes(), x, y);
        }
    }
    if let Some(icon) = a.pics.get(24) {
        obc_render::draw2d::draw_pic(fb, icon, right - 40, top + 2);
    }
}

/// A centred message box (`US_CenterWindow` + print), used for PAUSE.
pub fn draw_box(fb: &mut Framebuffer, a: &Assets, text: &str) {
    let Some(font) = a.fonts.first() else { return };
    let w = obc_render::draw2d::text_width(font, text.as_bytes()) + 16;
    let h = i32::from(font.height) + 10;
    let (x, y) = (160 - w / 2, 80 - h / 2);
    fb.bar(x - 1, y - 1, w + 2, h + 2, 0);
    fb.bar(x, y, w, h, 0x1C);
    obc_render::draw2d::draw_text(fb, font, text.as_bytes(), x + 8, y + 5, 0x0F);
}
