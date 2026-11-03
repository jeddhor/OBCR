//! Front end: intro and title (`DemoLoop` 1497:1442), the control panel
//! (`US_ControlPanel` 1618:061a, GAMEFLOW §4), help, quit and credits.
//! The OPTIONS screen holds the port's own settings (display, controls,
//! audio); it opens with O from the main menu, or from AUDIO MIX.

use std::sync::Arc;

use obc_audio::{AudioCommand, Pan, Volumes};
use obc_core::input::{InputState, pad, sc};
use obc_render::Framebuffer;
use obc_render::draw2d::{draw_pic, draw_text, text_width};
use obc_render::palette::{Palette, faded};

use crate::assets::Assets;
use crate::game::Game;
use crate::present::Aspect;
use crate::screen::{Ctx, Frame, Screen, Transition};
use crate::sound::DIGI_RATE;

/// Full-screen pictures (VGAGRAPH pic numbers).
mod pic {
    pub const STORY: usize = 1;
    pub const TITLE: usize = 2;
    pub const HELP: usize = 3;
    pub const MENU: usize = 4;
    pub const CITY: usize = 5;
    pub const CREDITS: usize = 6;
    pub const SPLAT: usize = 20;
    pub const CURSOR_OFF: usize = 22;
    pub const CURSOR_ON: usize = 23;
}

/// Menu sound (`SD_PlaySound(85)` per cursor step).
const MOVE_SOUND: u16 = 85;
/// `TicDelay(20)` between repeated cursor steps.
const REPEAT_TICS: u32 = 20;

/// Nearest palette entry to an 8-bit colour.
pub fn nearest(pal: &Palette, rgb: [u8; 3]) -> u8 {
    let d =
        |c: &[u8; 3]| -> i32 { (0..3).map(|i| (i32::from(c[i]) - i32::from(rgb[i])).pow(2)).sum() };
    (0..256).min_by_key(|&i| d(&pal[i])).unwrap_or(0) as u8
}

fn play_digi(ctx: &Ctx, id: u16) {
    if let Some(d) = ctx.assets.digi.get(usize::from(id)).filter(|d| !d.is_empty()) {
        ctx.audio.send(AudioCommand::PlayDigi {
            channel: 0,
            samples: Arc::from(d.as_slice()),
            rate: DIGI_RATE,
            pan: Pan::default(),
        });
    }
}

fn start_cp_music(ctx: &Ctx, song: usize) {
    if let Ok(m) = ctx.assets.audio.music(300 + song) {
        ctx.audio.send(AudioCommand::StartMusic(Arc::from(m)));
    }
}

fn any_key(input: &InputState) -> bool {
    input.events.iter().any(|e| e.down && !e.repeat) || input.pad.buttons & !input.pad_prev != 0
}

fn select(input: &InputState) -> bool {
    input.pressed(sc::ENTER)
        || input.pressed(sc::SPACE)
        || input.pad_pressed(pad::SOUTH | pad::START)
}

fn back(input: &InputState) -> bool {
    input.pressed(sc::ESCAPE) || input.pad_pressed(pad::EAST | pad::BACK)
}

/// A pixel order for `FizzleFade`: pixel (x, y) appears at frame `fizzle(x, y) % frames`.
fn fizzle(x: usize, y: usize) -> u32 {
    let mut h = (x as u32).wrapping_mul(0x9E37_79B1) ^ (y as u32).wrapping_mul(0x85EB_CA77);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B_3C6D);
    h ^ (h >> 12)
}

/// Draws a full-screen pic into the layer (no transparency).
fn show_pic(out: &mut Frame, a: &Assets, n: usize) {
    out.frame.clear(0);
    out.layer.clear(0);
    if let Some(p) = a.pics.get(n) {
        draw_pic(out.layer, p, 0, 0);
    }
    out.layer_key = None;
}

/// A centred message window on the 320×200 layer, picture palette colours.
pub fn message_box(fb: &mut Framebuffer, a: &Assets, pal: &Palette, lines: &[&str]) {
    let Some(font) = a.fonts.first() else { return };
    let lh = i32::from(font.height) + 2;
    let w = lines.iter().map(|l| text_width(font, l.as_bytes())).max().unwrap_or(0) + 20;
    let h = lh * lines.len() as i32 + 12;
    let (x, y) = (160 - w / 2, 100 - h / 2);
    fb.bar(x - 2, y - 2, w + 4, h + 4, nearest(pal, [255, 255, 255]));
    fb.bar(x, y, w, h, nearest(pal, [24, 24, 72]));
    let fg = nearest(pal, [255, 255, 255]);
    for (i, l) in lines.iter().enumerate() {
        let tw = text_width(font, l.as_bytes());
        draw_text(fb, font, l.as_bytes(), 160 - tw / 2, y + 6 + lh * i as i32, fg);
    }
}

// ---------------------------------------------------------------- title

/// Intro (pic 1), title (pic 2) with gunfire and splats, then the menu.
pub struct Title {
    stage: u8,
    t: u32,
    splats: Vec<(i32, i32, u32)>,
}

/// Fades in `DemoLoop` (`VL_FadeOut(…, 80)` unless stated).
const FADE: u32 = 80;

impl Title {
    pub fn new() -> Self {
        // Presentation-only random positions: the original's 24 US_RndT calls
        // precede NewGame, which reseeds nothing in single player, but OBCR's
        // simulation starts every game at rndindex 0 (DESIGN determinism).
        let mut rng = obc_core::rng::Rng::new();
        let mut r = || i32::from(rng.rnd_t());
        let mut splats = Vec::new();
        let mut when = 0u32;
        for i in 0..9 {
            let x = if i % 2 == 0 { r() % 125 } else { 300 - r() % 125 };
            let y = if i % 4 < 2 { r() % 60 } else { 180 - r() % 60 };
            splats.push((x, y, when));
            when += match i {
                1 | 5 => (r() % 10) as u32,
                2 | 6 => (r() % 20) as u32,
                3 | 7 => (r() % 30) as u32,
                _ => 0,
            } + 4;
        }
        Self { stage: 0, t: 0, splats }
    }
}

impl Default for Title {
    fn default() -> Self {
        Self::new()
    }
}

impl Screen for Title {
    fn tick(&mut self, ctx: &mut Ctx) -> Transition {
        if self.stage == 0 && self.t == 0 {
            start_cp_music(ctx, 8);
        }
        let key = any_key(ctx.input);
        self.t += 1;
        // Stages: 0 story fade-in (65), 1 story hold (275), 2 fade-out,
        // 3 title + sound (100), 4 splats, 5 hold (210), 6 fade-out.
        let next = match self.stage {
            0 => self.t >= 65 || key,
            1 => self.t >= 275 || key,
            2 => self.t >= FADE,
            3 => {
                if self.t == 1 {
                    play_digi(ctx, 86);
                }
                self.t >= 100 || key
            }
            4 => self.t > self.splats.last().map_or(0, |s| s.2) || key,
            5 => self.t >= 210 || key,
            _ => self.t >= FADE / 4 || key,
        };
        if next {
            if self.stage == 6 || (key && self.stage >= 3) {
                return Transition::Replace(main_menu(ctx.assets));
            }
            self.stage = if key && self.stage < 2 { 2 } else { self.stage + 1 };
            self.t = 0;
        }
        Transition::None
    }

    fn draw(&mut self, a: &Assets, out: &mut Frame) {
        let n = if self.stage < 3 { pic::STORY } else { pic::TITLE };
        show_pic(out, a, n);
        if self.stage >= 4
            && let Some(p) = a.pics.get(pic::SPLAT)
        {
            let shown = if self.stage == 4 { self.t } else { u32::MAX };
            for &(x, y, when) in &self.splats {
                if when <= shown {
                    obc_render::draw2d::draw_pic_masked(out.layer, p, x, y, 0);
                }
            }
        }
        out.palette = match self.stage {
            0 => faded(&a.picpal, 65 - self.t.min(65) as i32, 65, [0, 0, 0]),
            2 => faded(&a.picpal, self.t as i32, FADE as i32, [0, 0, 0]),
            6 => faded(&a.picpal, self.t as i32, (FADE / 4) as i32, [0, 0, 0]),
            _ => a.picpal,
        };
    }
}

// ---------------------------------------------------------------- control panel

struct Item {
    x: i32,
    y: i32,
    hotkey: u8,
}

const ITEMS: [Item; 10] = [
    Item { x: 283, y: 11, hotkey: 0x2F },  // VIEW SIZE (V)
    Item { x: 283, y: 25, hotkey: 0x19 },  // GAME TYPE (P)
    Item { x: 283, y: 41, hotkey: 0x23 },  // HIGH SCORES (H)
    Item { x: 283, y: 56, hotkey: 0x32 },  // AUDIO MIX (M)
    Item { x: 283, y: 85, hotkey: 0x31 },  // START GAME (N)
    Item { x: 283, y: 112, hotkey: 0x2E }, // CONTINUE (C)
    Item { x: 283, y: 127, hotkey: 0x1E }, // ABORT GAME (A)
    Item { x: 283, y: 142, hotkey: 0x10 }, // EXIT TO DOS (Q)
    Item { x: 283, y: 171, hotkey: 0x26 }, // LOAD GAME (L)
    Item { x: 283, y: 185, hotkey: 0x1F }, // SAVE GAME (S)
];

const VIEW_SIZE: usize = 0;
const GAME_TYPE: usize = 1;
const HIGH_SCORES: usize = 2;
const AUDIO_MIX: usize = 3;
const START_GAME: usize = 4;
const CONTINUE: usize = 5;
const ABORT_GAME: usize = 6;
const EXIT: usize = 7;
const LOAD_GAME: usize = 8;
const SAVE_GAME: usize = 9;

#[derive(Debug, Clone, PartialEq)]
enum Mode {
    Items,
    ViewSize,
    GameType,
    Confirm(usize),
    Message(&'static str, u32),
    Help(u32),
    FadeIn(u32),
}

pub struct Menu {
    in_game: bool,
    cur: usize,
    blink: u32,
    repeat: u32,
    mode: Mode,
    /// Copies of the settings shown on the menu, refreshed every tic.
    viewsize: u16,
    gametype: i16,
}

/// The main menu, out of game.
pub fn main_menu(_a: &Assets) -> Box<dyn Screen> {
    Box::new(Menu::new(false))
}

/// The control panel over a running game (Esc).
pub fn in_game(_a: &Assets) -> Box<dyn Screen> {
    Box::new(Menu::new(true))
}

impl Menu {
    fn new(in_game: bool) -> Self {
        Self {
            in_game,
            cur: if in_game { CONTINUE } else { START_GAME },
            blink: 0,
            repeat: 0,
            mode: Mode::FadeIn(0),
            viewsize: 18,
            gametype: 1,
        }
    }

    fn active(&self, i: usize) -> bool {
        match i {
            GAME_TYPE | START_GAME => !self.in_game,
            CONTINUE | ABORT_GAME | SAVE_GAME => self.in_game,
            LOAD_GAME => !self.in_game,
            _ => true,
        }
    }

    fn step(&mut self, dir: i32) {
        let mut i = self.cur as i32;
        for _ in 0..ITEMS.len() {
            i = (i + dir).rem_euclid(ITEMS.len() as i32);
            if self.active(i as usize) {
                break;
            }
        }
        self.cur = i as usize;
        self.blink = 0;
    }

    fn activate(&mut self, ctx: &mut Ctx) -> Transition {
        match self.cur {
            VIEW_SIZE => self.mode = Mode::ViewSize,
            GAME_TYPE => self.mode = Mode::GameType,
            HIGH_SCORES => self.mode = Mode::Message("HIGH SCORES: COMING SOON", 105),
            AUDIO_MIX => return Transition::Push(Box::new(Options::new(true))),
            START_GAME => {
                ctx.audio.send(AudioCommand::StopMusic);
                let gametype = ctx.settings.game.gametype.clamp(0, 3);
                return Transition::Reset(Box::new(Game::new(ctx, gametype)));
            }
            CONTINUE => return Transition::Pop,
            ABORT_GAME | EXIT => self.mode = Mode::Confirm(self.cur),
            LOAD_GAME | SAVE_GAME => self.mode = Mode::Message("NOT AVAILABLE YET", 105),
            _ => {}
        }
        Transition::None
    }
}

impl Screen for Menu {
    fn tick(&mut self, ctx: &mut Ctx) -> Transition {
        let input = ctx.input;
        ctx.requests.relative_mouse = Some(false);
        if matches!(self.mode, Mode::FadeIn(0)) {
            start_cp_music(ctx, 0);
        }
        self.blink += 1;
        if self.blink > 535 {
            self.blink = 501;
        }
        let up = input.down(sc::UP) || input.pad.held(pad::DPAD_UP) || input.pad.left_y < -0.6;
        let down = input.down(sc::DOWN) || input.pad.held(pad::DPAD_DOWN) || input.pad.left_y > 0.6;
        let dir = i32::from(down) - i32::from(up);
        let fresh = input.pressed(sc::UP)
            || input.pressed(sc::DOWN)
            || input.pad_pressed(pad::DPAD_UP | pad::DPAD_DOWN);
        if dir == 0 {
            self.repeat = 0;
        } else if fresh {
            self.repeat = REPEAT_TICS;
        }
        let edge = dir != 0
            && (fresh || {
                self.repeat = self.repeat.saturating_sub(1);
                self.repeat == 0
            });
        if edge && !fresh {
            self.repeat = REPEAT_TICS;
        }

        let t = self.tick_mode(ctx, dir, edge);
        self.viewsize = ctx.settings.game.viewsize;
        self.gametype = ctx.settings.game.gametype;
        t
    }

    fn draw(&mut self, a: &Assets, out: &mut Frame) {
        self.draw_menu(a, out);
    }
}

impl Menu {
    fn tick_mode(&mut self, ctx: &mut Ctx, dir: i32, edge: bool) -> Transition {
        let input = ctx.input;
        match self.mode.clone() {
            Mode::FadeIn(t) => {
                self.mode = if t >= 10 { Mode::Items } else { Mode::FadeIn(t + 1) };
            }
            Mode::Items => {
                if edge {
                    self.step(dir);
                    play_digi(ctx, MOVE_SOUND);
                }
                if back(input) {
                    if self.in_game {
                        return Transition::Pop;
                    }
                    self.mode = Mode::Confirm(EXIT);
                    return Transition::None;
                }
                if input.pressed(sc::H) && !self.in_game {
                    self.mode = Mode::Help(0);
                    return Transition::None;
                }
                if input.pressed(sc::O) {
                    return Transition::Push(Box::new(Options::new(false)));
                }
                if let Some(i) = ITEMS
                    .iter()
                    .enumerate()
                    .position(|(i, it)| input.pressed(it.hotkey) && self.active(i))
                {
                    self.cur = i;
                    return self.activate(ctx);
                }
                if select(input) {
                    return self.activate(ctx);
                }
            }
            Mode::ViewSize => {
                let v = &mut ctx.settings.game.viewsize;
                // Up enlarges; Down or Right shrinks (the original's quirk).
                if input.pressed(sc::UP) || input.pad_pressed(pad::DPAD_UP) {
                    *v = (*v + 2).min(20);
                }
                if input.pressed(sc::DOWN)
                    || input.pressed(sc::RIGHT)
                    || input.pad_pressed(pad::DPAD_DOWN)
                {
                    *v = v.saturating_sub(2).max(12);
                }
                if back(input) || select(input) {
                    self.mode = Mode::Items;
                }
            }
            Mode::GameType => {
                let g = &mut ctx.settings.game.gametype;
                if input.pressed(sc::UP) || input.pad_pressed(pad::DPAD_UP) {
                    *g = (*g + 1).min(3);
                }
                if input.pressed(sc::DOWN)
                    || input.pressed(sc::RIGHT)
                    || input.pad_pressed(pad::DPAD_DOWN)
                {
                    *g = (*g - 1).max(0);
                }
                if back(input) || select(input) {
                    self.mode = Mode::Items;
                }
            }
            Mode::Confirm(what) => {
                if input.pressed(sc::Y) || input.pad_pressed(pad::SOUTH) {
                    return match what {
                        EXIT => {
                            ctx.audio.send(AudioCommand::StopMusic);
                            Transition::Reset(Box::new(QuitScreen::default()))
                        }
                        _ => Transition::Reset(main_menu(ctx.assets)),
                    };
                }
                if input.pressed(sc::N) || back(input) {
                    self.mode = Mode::Items;
                }
            }
            Mode::Message(m, t) => {
                self.mode =
                    if t == 0 || any_key(input) { Mode::Items } else { Mode::Message(m, t - 1) };
            }
            Mode::Help(t) => {
                if t > 5 && any_key(input) {
                    self.mode = Mode::Items;
                } else {
                    self.mode = Mode::Help(t + 1);
                }
            }
        }
        Transition::None
    }

    fn draw_menu(&mut self, a: &Assets, out: &mut Frame) {
        let pal = a.picpal;
        if let Mode::Help(t) = self.mode {
            show_pic(out, a, pic::MENU);
            if let Some(h) = a.pics.get(pic::HELP) {
                // FizzleFade over 5 frames (2 tics each).
                let shown = (t / 2).min(5);
                for y in 0..200 {
                    for x in 0..320 {
                        if fizzle(x, y) % 5 < shown {
                            out.layer.pixels[y * 320 + x] = h.pixels[y * 320 + x];
                        }
                    }
                }
            }
            out.palette = pal;
            return;
        }
        show_pic(out, a, pic::MENU);
        // Cursor blink (HandleMenu): on, then off at 525–535, repeating from 501.
        let lit = !(525..=535).contains(&self.blink);
        for (i, it) in ITEMS.iter().enumerate() {
            let p = if i == self.cur && lit { pic::CURSOR_ON } else { pic::CURSOR_OFF };
            if let Some(c) = a.pics.get(p) {
                draw_pic(out.layer, c, it.x, it.y);
            }
        }
        let fg = nearest(&pal, [255, 255, 255]);
        let red = nearest(&pal, [255, 40, 40]);
        if let Some(font) = a.fonts.first() {
            // VIEW SIZE at (294/297, 9) and GAME TYPE at (298, 23).
            let vs = match self.viewsize {
                12 => "XS",
                14 => "S",
                16 => "M",
                18 => "L",
                20 => "XL",
                _ => "??",
            };
            let x = if vs.len() > 1 { 294 } else { 297 };
            let c = if self.mode == Mode::ViewSize { fg } else { red };
            draw_text(out.layer, font, vs.as_bytes(), x, 9, c);
            let gt = ["1", "2", "3", "R"][self.gametype.clamp(0, 3) as usize];
            let c = if self.mode == Mode::GameType { fg } else { red };
            draw_text(out.layer, font, gt.as_bytes(), 298, 23, c);
            if matches!(self.mode, Mode::ViewSize | Mode::GameType) {
                draw_text(out.layer, font, b"UP / DOWN", 4, 186, fg);
            } else if !self.in_game {
                draw_text(out.layer, font, b"O  OPTIONS", 4, 186, fg);
            }
        }
        match &self.mode {
            Mode::Confirm(EXIT) => message_box(out.layer, a, &pal, &["BACK TO DOS ", "Y/N"]),
            Mode::Confirm(_) => message_box(out.layer, a, &pal, &["ABORT THIS GAME Y/N "]),
            Mode::Message(m, _) => message_box(out.layer, a, &pal, &[m]),
            _ => {}
        }
        out.palette = match self.mode {
            Mode::FadeIn(t) => faded(&pal, 10 - t.min(10) as i32, 10, [0, 0, 0]),
            _ => pal,
        };
    }
}

// ---------------------------------------------------------------- quit

/// `Quit(NULL)`: pic 5, then the credits (pic 6) by fizzle, then exit.
#[derive(Default)]
pub struct QuitScreen {
    t: u32,
}

impl Screen for QuitScreen {
    fn tick(&mut self, ctx: &mut Ctx) -> Transition {
        self.t += 1;
        if self.t > 50 + 70 + 50 + 70 + 30 || (self.t > 50 && any_key(ctx.input) && self.t > 170) {
            return Transition::Quit;
        }
        Transition::None
    }

    fn draw(&mut self, a: &Assets, out: &mut Frame) {
        let t = self.t;
        show_pic(out, a, pic::CITY);
        if t > 120
            && let Some(c) = a.pics.get(pic::CREDITS)
        {
            // FizzleFade over 25 frames (2 tics each).
            let shown = ((t - 120) / 2).min(25);
            for y in 0..200 {
                for x in 0..320 {
                    if fizzle(x, y) % 25 < shown {
                        out.layer.pixels[y * 320 + x] = c.pixels[y * 320 + x];
                    }
                }
            }
        }
        out.palette = if t < 50 {
            faded(&a.picpal, 50 - t as i32, 50, [0, 0, 0])
        } else if t > 240 {
            faded(&a.picpal, (t - 240) as i32, 30, [0, 0, 0])
        } else {
            a.picpal
        };
    }
}

// ---------------------------------------------------------------- options

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Opt {
    Display,
    Scale,
    Aspect,
    Fullscreen,
    Smooth,
    IntegerScale,
    Controls,
    AlwaysRun,
    Mouse,
    PadTurn,
    Rumble,
    Master,
    Sfx,
    Music,
    Channels,
}

const OPTS: [Opt; 15] = [
    Opt::Display,
    Opt::Scale,
    Opt::Aspect,
    Opt::Fullscreen,
    Opt::Smooth,
    Opt::IntegerScale,
    Opt::Controls,
    Opt::AlwaysRun,
    Opt::Mouse,
    Opt::PadTurn,
    Opt::Rumble,
    Opt::Master,
    Opt::Sfx,
    Opt::Music,
    Opt::Channels,
];

/// The port's settings: Classic or Enhanced display, resolution, aspect,
/// controls and audio. Left/Right change a value; Esc returns.
pub struct Options {
    cur: usize,
    shown: crate::settings::Settings,
}

impl Options {
    pub fn new(audio_first: bool) -> Self {
        Self { cur: if audio_first { 11 } else { 0 }, shown: Default::default() }
    }

    fn label(o: Opt) -> &'static str {
        match o {
            Opt::Display => "DISPLAY",
            Opt::Scale => "RESOLUTION",
            Opt::Aspect => "ASPECT",
            Opt::Fullscreen => "FULLSCREEN",
            Opt::Smooth => "SMOOTH FILTER",
            Opt::IntegerScale => "INTEGER SCALING",
            Opt::Controls => "CONTROLS",
            Opt::AlwaysRun => "ALWAYS RUN",
            Opt::Mouse => "MOUSE SPEED",
            Opt::PadTurn => "PAD TURN SPEED",
            Opt::Rumble => "RUMBLE",
            Opt::Master => "MASTER VOLUME",
            Opt::Sfx => "SOUND FX VOLUME",
            Opt::Music => "MUSIC VOLUME",
            Opt::Channels => "SOUND VOICES",
        }
    }

    fn value(o: Opt, s: &crate::settings::Settings) -> String {
        let on = |b: bool| if b { "ON".to_string() } else { "OFF".to_string() };
        let pct = |v: f32| format!("{}", (v * 100.0).round() as i32);
        let p = s.video.presentation();
        match o {
            Opt::Display => if p.is_classic() { "CLASSIC 320X200" } else { "ENHANCED" }.into(),
            Opt::Scale => format!("{}X  {}", s.video.scale, 200 * s.video.scale),
            Opt::Aspect => s.video.aspect.clone(),
            Opt::Fullscreen => on(s.video.fullscreen),
            Opt::Smooth => on(s.video.smooth),
            Opt::IntegerScale => on(s.video.integer_scale),
            Opt::Controls => s.input.profile.to_uppercase(),
            Opt::AlwaysRun => on(s.input.always_run),
            Opt::Mouse => pct(s.input.mouse_sensitivity),
            Opt::PadTurn => pct(s.input.gamepad_turn_speed),
            Opt::Rumble => on(s.input.rumble),
            Opt::Master => pct(s.audio.master),
            Opt::Sfx => pct(s.audio.sfx),
            Opt::Music => pct(s.audio.music),
            Opt::Channels => {
                if s.audio.channels <= 1 {
                    "1 ORIGINAL".into()
                } else {
                    format!("{} MIXED", s.audio.channels)
                }
            }
        }
    }

    fn change(o: Opt, d: i32, ctx: &mut Ctx) {
        let s = &mut *ctx.settings;
        let step = |v: &mut f32, lo: f32, hi: f32, st: f32| {
            *v = ((*v + st * d as f32) * 20.0).round() / 20.0;
            *v = v.clamp(lo, hi);
        };
        match o {
            Opt::Display => {
                s.video.mode =
                    if s.video.mode == "classic" { "enhanced" } else { "classic" }.into();
            }
            Opt::Scale => s.video.scale = (s.video.scale as i32 + d).clamp(2, 8) as usize,
            Opt::Aspect => {
                let all = [Aspect::Standard, Aspect::Wide16x10, Aspect::Wide16x9, Aspect::Wide21x9];
                let cur = all.iter().position(|a| a.label() == s.video.aspect).unwrap_or(2);
                let n = (cur as i32 + d).rem_euclid(all.len() as i32) as usize;
                s.video.aspect = all[n].label().into();
            }
            Opt::Fullscreen => {
                s.video.fullscreen = !s.video.fullscreen;
                ctx.requests.fullscreen = Some(s.video.fullscreen);
            }
            Opt::Smooth => {
                s.video.smooth = !s.video.smooth;
                ctx.requests.apply_video = true;
            }
            Opt::IntegerScale => {
                s.video.integer_scale = !s.video.integer_scale;
                ctx.requests.apply_video = true;
            }
            Opt::Controls => {
                s.input.profile =
                    if s.input.profile == "modern" { "original" } else { "modern" }.into();
            }
            Opt::AlwaysRun => s.input.always_run = !s.input.always_run,
            Opt::Mouse => step(&mut s.input.mouse_sensitivity, 0.25, 4.0, 0.25),
            Opt::PadTurn => step(&mut s.input.gamepad_turn_speed, 0.25, 3.0, 0.25),
            Opt::Rumble => s.input.rumble = !s.input.rumble,
            Opt::Master => step(&mut s.audio.master, 0.0, 1.0, 0.1),
            Opt::Sfx => step(&mut s.audio.sfx, 0.0, 1.0, 0.1),
            Opt::Music => step(&mut s.audio.music, 0.0, 1.0, 0.1),
            Opt::Channels => s.audio.channels = if s.audio.channels <= 1 { 4 } else { 1 },
        }
        let a = &s.audio;
        ctx.audio.send(AudioCommand::SetVolumes(Volumes {
            master: a.master,
            sfx: a.sfx,
            music: a.music,
        }));
    }
}

impl Screen for Options {
    fn tick(&mut self, ctx: &mut Ctx) -> Transition {
        let input = ctx.input;
        ctx.requests.relative_mouse = Some(false);
        if back(input) {
            return Transition::Pop;
        }
        let pressed_dir = |neg: bool| {
            if neg {
                input.pressed(sc::LEFT) || input.pad_pressed(pad::DPAD_LEFT)
            } else {
                input.pressed(sc::RIGHT)
                    || input.pressed(sc::ENTER)
                    || input.pad_pressed(pad::DPAD_RIGHT | pad::SOUTH)
            }
        };
        if input.pressed(sc::UP) || input.pad_pressed(pad::DPAD_UP) {
            self.cur = (self.cur + OPTS.len() - 1) % OPTS.len();
            play_digi(ctx, MOVE_SOUND);
        }
        if input.pressed(sc::DOWN) || input.pad_pressed(pad::DPAD_DOWN) {
            self.cur = (self.cur + 1) % OPTS.len();
            play_digi(ctx, MOVE_SOUND);
        }
        let d = if pressed_dir(true) {
            -1
        } else if pressed_dir(false) {
            1
        } else {
            0
        };
        if d != 0 {
            Self::change(OPTS[self.cur], d, ctx);
        }
        self.shown = ctx.settings.clone();
        Transition::None
    }

    fn draw(&mut self, a: &Assets, out: &mut Frame) {
        out.frame.clear(0);
        out.layer.clear(0);
        out.layer_key = None;
        let pal = a.gamepal;
        let (fg, hi, dim) = (
            nearest(&pal, [255, 255, 255]),
            nearest(&pal, [255, 220, 0]),
            nearest(&pal, [150, 150, 150]),
        );
        out.layer.bar(8, 4, 304, 192, nearest(&pal, [16, 16, 56]));
        let Some(font) = a.fonts.first() else { return };
        draw_text(out.layer, font, b"OPTIONS", 136, 7, hi);
        for (i, &o) in OPTS.iter().enumerate() {
            let y = 22 + i as i32 * 11;
            let c = if i == self.cur { hi } else { fg };
            draw_text(out.layer, font, Self::label(o).as_bytes(), 20, y, c);
            let v = Self::value(o, &self.shown);
            draw_text(out.layer, font, v.as_bytes(), 180, y, if i == self.cur { hi } else { dim });
        }
        out.palette = pal;
    }
}
