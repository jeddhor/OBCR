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
use obc_sim::world::World;

use crate::assets::Assets;
use crate::game::Game;
use crate::present::Aspect;
use crate::saves;
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
    pub const VICTORY: usize = 7;
    pub const CITY_BLUE: usize = 8;
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
    fn name(&self) -> &'static str {
        "title"
    }

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
    /// Pic 3 (the original keys), then OBCR's bindings for the current profile.
    Help(u32),
    Keys(bool),
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
    /// The game as it was when the panel opened, for SAVE GAME.
    world: Option<Box<World>>,
}

/// The main menu, out of game.
pub fn main_menu(_a: &Assets) -> Box<dyn Screen> {
    Box::new(Menu::new(false))
}

/// The control panel over a running game (Esc), holding a copy of the world.
pub fn in_game(world: Box<World>) -> Box<dyn Screen> {
    let mut m = Menu::new(true);
    m.world = Some(world);
    Box::new(m)
}

impl Menu {
    fn new(in_game: bool) -> Self {
        Self {
            world: None,
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
            CONTINUE | ABORT_GAME => self.in_game,
            // Game types 4–6 cannot be saved or loaded (GAMEFLOW §4).
            SAVE_GAME => {
                self.in_game && self.world.as_ref().is_some_and(|w| w.gamestate.difficulty < 4)
            }
            LOAD_GAME => !self.in_game && self.gametype < 4,
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
            HIGH_SCORES => {
                return Transition::Push(Box::new(HighScores::new(ctx, None).view_only()));
            }
            AUDIO_MIX => return Transition::Push(Box::new(Options::new(true))),
            START_GAME => {
                let gametype = ctx.settings.game.gametype.clamp(0, 6);
                if gametype >= 4 {
                    // NetSetup (358f:0959); Esc there returns to this menu.
                    return Transition::Push(Box::new(crate::netui::NetSetup::new(ctx, gametype)));
                }
                ctx.audio.send(AudioCommand::StopMusic);
                return Transition::Reset(Box::new(Game::new(ctx, gametype)));
            }
            CONTINUE => return Transition::Pop,
            ABORT_GAME | EXIT => self.mode = Mode::Confirm(self.cur),
            LOAD_GAME => return Transition::Push(Box::new(SlotMenu::new(ctx, None))),
            SAVE_GAME => {
                if let Some(w) = &self.world {
                    return Transition::Push(Box::new(SlotMenu::new(ctx, Some(w.clone()))));
                }
            }
            _ => {}
        }
        Transition::None
    }
}

impl Screen for Menu {
    fn name(&self) -> &'static str {
        "menu"
    }

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
                    *g = (*g + 1).min(6);
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
            Mode::Help(t) => {
                self.mode = if t > 5 && any_key(input) {
                    Mode::Keys(ctx.settings.input.profile != "original")
                } else {
                    Mode::Help(t + 1)
                };
            }
            Mode::Keys(_) => {
                if any_key(input) {
                    self.mode = Mode::Items;
                }
            }
        }
        Transition::None
    }

    fn draw_menu(&mut self, a: &Assets, out: &mut Frame) {
        let pal = a.picpal;
        if let Mode::Keys(modern) = self.mode {
            draw_keys(out, a, modern);
            return;
        }
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
            let gt = ["1", "2", "3", "R", "N", "T", "M"][self.gametype.clamp(0, 6) as usize];
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
            _ => {}
        }
        out.palette = match self.mode {
            Mode::FadeIn(t) => faded(&pal, 10 - t.min(10) as i32, 10, [0, 0, 0]),
            _ => pal,
        };
    }
}

/// The second help page: OBCR's bindings (keyboard profile and gamepad).
fn draw_keys(out: &mut Frame, a: &Assets, modern: bool) {
    out.frame.clear(0);
    out.layer.clear(0);
    out.layer_key = None;
    let pal = a.gamepal;
    out.layer.bar(4, 2, 312, 196, nearest(&pal, [16, 16, 56]));
    let Some(font) = a.fonts.first() else { return };
    let (hi, fg) = (nearest(&pal, [255, 220, 0]), nearest(&pal, [255, 255, 255]));
    let title: &[u8] = if modern { b"MODERN CONTROLS" } else { b"ORIGINAL CONTROLS" };
    draw_text(out.layer, font, title, 160 - text_width(font, title) / 2, 5, hi);
    let rows: &[(&str, &str, &str)] = if modern {
        &[
            ("MOVE", "W S  ARROWS", "LEFT STICK"),
            ("TURN", "MOUSE  ARROWS", "RIGHT STICK"),
            ("STRAFE", "A D", "LEFT STICK"),
            ("FIRE", "LMB  CTRL", "R TRIGGER"),
            ("USE", "SPACE E RMB", "A"),
            ("RUN", "SHIFT", "L TRIGGER"),
            ("WEAPON", "1-5  WHEEL", "LB RB"),
            ("ALLY SWITCH", "Q", "X"),
            ("STAY+SWITCH", "Z", "B"),
            ("ALLY MODE", "F", "Y"),
            ("WIPE VISOR", "V", "DPAD DOWN"),
            ("MAP", "TAB", "BACK"),
            ("MENU", "ESC", "START"),
        ]
    } else {
        &[
            ("MOVE", "UP DOWN", "LEFT STICK"),
            ("TURN", "LEFT RIGHT MOUSE", "RIGHT STICK"),
            ("STRAFE", "ALT", "LEFT STICK"),
            ("FIRE", "CTRL  LMB", "R TRIGGER"),
            ("USE", "SPACE  MMB", "A"),
            ("RUN", "SHIFT  RMB", "L TRIGGER"),
            ("WEAPON", "1-5", "LB RB"),
            ("ALLY SWITCH", "A", "X"),
            ("STAY+SWITCH", "S", "B"),
            ("ALLY MODE", "F", "Y"),
            ("WIPE VISOR", "W", "DPAD DOWN"),
            ("MAP", "TAB", "BACK"),
            ("MENU", "ESC", "START"),
        ]
    };
    for (i, (what, keys, pad)) in rows.iter().enumerate() {
        let y = 22 + 13 * i as i32;
        draw_text(out.layer, font, what.as_bytes(), 10, y, hi);
        draw_text(out.layer, font, keys.as_bytes(), 110, y, fg);
        draw_text(out.layer, font, pad.as_bytes(), 226, y, fg);
    }
    out.palette = pal;
}

// ---------------------------------------------------------------- load / save

/// Text entry for save names and high scores (`US_LineInput`).
#[derive(Debug, Clone, Default)]
struct LineInput {
    text: String,
    max: usize,
}

impl LineInput {
    /// Applies this tic's typing. Returns Some(true) on Enter, Some(false) on Esc.
    fn update(&mut self, input: &InputState) -> Option<bool> {
        for c in input.text.chars() {
            if (' '..='~').contains(&c) && self.text.len() < self.max {
                self.text.push(c);
            }
        }
        if input.pressed(sc::BACKSPACE) {
            self.text.pop();
        }
        if input.pressed(sc::ENTER) {
            return Some(true);
        }
        if input.pressed(sc::ESCAPE) {
            return Some(false);
        }
        None
    }
}

/// LOAD GAME / SAVE GAME slot list (GAMEFLOW §9.1): ten slots, the last is
/// AutoSave (loadable, not writable from the menu).
pub struct SlotMenu {
    world: Option<Box<World>>,
    slots: Vec<Option<obc_sim::save::SaveHeader>>,
    cur: usize,
    editing: Option<LineInput>,
    error: Option<(String, u32)>,
}

impl SlotMenu {
    /// `world` set: save it; otherwise load.
    fn new(ctx: &Ctx, world: Option<Box<World>>) -> Self {
        let slots = saves::list(ctx.save_dir);
        let cur = if world.is_some() {
            ctx.settings.game.quicksave_slot.map_or(0, usize::from).min(saves::AUTOSAVE - 1)
        } else {
            slots.iter().position(Option::is_some).unwrap_or(0)
        };
        Self { world, slots, cur, editing: None, error: None }
    }

    fn saving(&self) -> bool {
        self.world.is_some()
    }

    fn label(&self, i: usize) -> String {
        match &self.slots[i] {
            Some(h) if i == saves::AUTOSAVE => format!("AutoSave  FLOOR {}", h.mapon + 1),
            Some(h) => format!("{}  FLOOR {}", h.name, h.mapon + 1),
            None if i == saves::AUTOSAVE => "AutoSave".into(),
            None if self.saving() => "available".into(),
            None => "empty".into(),
        }
    }

    /// Writes the held world to the current slot; true on success.
    fn save(&mut self, ctx: &mut Ctx, name: &str) -> bool {
        let Some(w) = &self.world else { return false };
        match saves::write(ctx.save_dir, self.cur, w, name, ctx.assets) {
            Ok(()) => {
                ctx.settings.game.quicksave_slot = Some(self.cur as u8);
                true
            }
            Err(e) => {
                self.error = Some((format!("CANT SAVE: {e}"), 175));
                false
            }
        }
    }
}

impl Screen for SlotMenu {
    fn name(&self) -> &'static str {
        "slots"
    }

    fn tick(&mut self, ctx: &mut Ctx) -> Transition {
        let input = ctx.input;
        ctx.requests.relative_mouse = Some(false);
        if let Some((_, t)) = &mut self.error {
            *t = t.saturating_sub(1);
            if *t == 0 || any_key(input) {
                self.error = None;
            }
            return Transition::None;
        }
        if let Some(edit) = &mut self.editing {
            ctx.requests.text_input = Some(true);
            match edit.update(input) {
                Some(true) if !edit.text.trim().is_empty() => {
                    let name = edit.text.trim().to_string();
                    self.editing = None;
                    ctx.requests.text_input = Some(false);
                    return if self.save(ctx, &name) { Transition::Pop } else { Transition::None };
                }
                Some(false) => {
                    self.editing = None;
                    ctx.requests.text_input = Some(false);
                }
                _ => {}
            }
            return Transition::None;
        }
        if back(input) {
            return Transition::Pop;
        }
        let last = if self.saving() { saves::AUTOSAVE - 1 } else { saves::AUTOSAVE };
        if input.pressed(sc::UP) || input.pad_pressed(pad::DPAD_UP) {
            self.cur = if self.cur == 0 { last } else { self.cur - 1 };
            play_digi(ctx, MOVE_SOUND);
        }
        if input.pressed(sc::DOWN) || input.pad_pressed(pad::DPAD_DOWN) {
            self.cur = if self.cur >= last { 0 } else { self.cur + 1 };
            play_digi(ctx, MOVE_SOUND);
        }
        if !select(input) {
            return Transition::None;
        }
        if self.saving() {
            let text = self.slots[self.cur].as_ref().map(|h| h.name.clone()).unwrap_or_default();
            if input.pad_pressed(pad::SOUTH | pad::START) {
                // A gamepad has no keyboard: keep the slot's name or number it.
                let name = if text.is_empty() { format!("SLOT {}", self.cur + 1) } else { text };
                return if self.save(ctx, &name) { Transition::Pop } else { Transition::None };
            }
            self.editing = Some(LineInput { text, max: saves::NAME_MAX });
            ctx.requests.text_input = Some(true);
        } else if self.slots[self.cur].is_some() {
            return match saves::read(ctx.save_dir, self.cur, ctx.assets) {
                Ok((_, w)) => {
                    ctx.audio.send(AudioCommand::StopMusic);
                    Transition::Reset(Box::new(Game::from_world(ctx, w)))
                }
                Err(e) => {
                    self.error = Some((format!("CANT LOAD: {e}"), 175));
                    Transition::None
                }
            };
        }
        Transition::None
    }

    fn draw(&mut self, a: &Assets, out: &mut Frame) {
        show_pic(out, a, pic::CITY_BLUE);
        let pal = a.picpal;
        let fg = nearest(&pal, [255, 255, 255]);
        let hi = nearest(&pal, [255, 220, 0]);
        let dim = nearest(&pal, [140, 140, 170]);
        let Some(font) = a.fonts.first() else { return };
        let title: &[u8] = if self.saving() { b"SAVE GAME" } else { b"LOAD GAME" };
        let tw = text_width(font, title);
        draw_text(out.layer, font, title, 160 - tw / 2, 4, hi);
        for i in 0..saves::SLOTS {
            let y = 24 + 17 * i as i32;
            let inactive = self.saving() && i == saves::AUTOSAVE;
            let c = if inactive {
                dim
            } else if i == self.cur {
                hi
            } else {
                fg
            };
            if i == self.cur
                && let Some(p) = a.pics.get(pic::CURSOR_ON)
            {
                draw_pic(out.layer, p, 18, y + 2);
            }
            let text = match &self.editing {
                Some(e) if i == self.cur => format!("{}_", e.text),
                _ => self.label(i),
            };
            out.layer.bar(32, y, 270, 14, nearest(&pal, [16, 16, 48]));
            draw_text(out.layer, font, text.as_bytes(), 36, y + 1, c);
        }
        if let Some((m, _)) = &self.error {
            message_box(out.layer, a, &pal, &[m.as_str()]);
        }
        out.palette = pal;
    }
}

// ---------------------------------------------------------------- high scores

/// The default table in `COUNT.EXE` (file 0x383E4; DATA_FORMATS §7).
const DEFAULT_SCORES: usize = 0x383E4;

fn default_scores(a: &Assets) -> Vec<crate::settings::Score> {
    a.exe
        .file(DEFAULT_SCORES, 7 * 28)
        .ok()
        .and_then(|b| obc_formats::config::parse_high_scores(b).ok())
        .unwrap_or_default()
        .into_iter()
        .map(|h| crate::settings::Score { name: h.name, score: h.score, completed: h.completed })
        .collect()
}

/// `DrawHighScores(enter)` (2f4d:1422, GAMEFLOW §8) on pic 8. With a new
/// qualifying score, its row is boxed and the name typed (18 characters).
pub struct HighScores {
    scores: Vec<crate::settings::Score>,
    editing: Option<(usize, LineInput)>,
    then_menu: bool,
    t: u32,
}

impl HighScores {
    /// `entry`: the finished game's score and floor, if it may enter the table.
    pub fn new(ctx: &mut Ctx, entry: Option<(i32, u16)>) -> Self {
        if ctx.settings.game.scores.len() != 7 {
            ctx.settings.game.scores = default_scores(ctx.assets);
        }
        let mut scores = ctx.settings.game.scores.clone();
        let mut editing = None;
        if let Some((score, completed)) = entry
            && let Some(i) = scores.iter().position(|s| s.score <= score)
        {
            scores.insert(i, crate::settings::Score { name: String::new(), score, completed });
            scores.truncate(7);
            editing = Some((i, LineInput { text: String::new(), max: 18 }));
        }
        Self { scores, editing, then_menu: true, t: 0 }
    }

    fn view_only(mut self) -> Self {
        self.then_menu = false;
        self
    }

    fn leave(&self, ctx: &Ctx) -> Transition {
        if self.then_menu { Transition::Reset(main_menu(ctx.assets)) } else { Transition::Pop }
    }
}

impl Screen for HighScores {
    fn name(&self) -> &'static str {
        "high-scores"
    }

    fn tick(&mut self, ctx: &mut Ctx) -> Transition {
        self.t += 1;
        ctx.requests.relative_mouse = Some(false);
        if let Some((i, edit)) = &mut self.editing {
            ctx.requests.text_input = Some(true);
            let done =
                edit.update(ctx.input).is_some() || ctx.input.pad_pressed(pad::SOUTH | pad::START);
            if done {
                let name = if edit.text.trim().is_empty() {
                    "PLAYER".to_string()
                } else {
                    edit.text.clone()
                };
                self.scores[*i].name = name;
                ctx.settings.game.scores = self.scores.clone();
                self.editing = None;
                ctx.requests.text_input = Some(false);
                self.t = 0;
            }
            return Transition::None;
        }
        if (self.t > 10 && any_key(ctx.input)) || self.t > 1000 {
            return self.leave(ctx);
        }
        Transition::None
    }

    fn draw(&mut self, a: &Assets, out: &mut Frame) {
        show_pic(out, a, pic::CITY_BLUE);
        let pal = a.picpal;
        let Some(font) = a.fonts.first() else { return };
        let white = nearest(&pal, [255, 255, 255]);
        let title = b"HIGH SCORES";
        draw_text(out.layer, font, title, 160 - text_width(font, title) / 2, 20, white);
        draw_text(out.layer, font, b"NAME", 24, 43, white);
        draw_text(out.layer, font, b"L", 210, 43, white);
        draw_text(out.layer, font, b"SCORE", 246, 43, white);
        for (i, s) in self.scores.iter().enumerate() {
            let y = 62 + 18 * i as i32;
            // Each row its own colours ('W' − 2i and 'o' − 2i).
            let (c1, c2) = (b'W' - 2 * i as u8, b'o' - 2 * i as u8);
            let editing = self.editing.as_ref().filter(|(r, _)| *r == i);
            if editing.is_some() {
                out.layer.bar(20, y - 2, 270, 16, 0x6F);
                out.layer.bar(21, y - 1, 268, 14, 0x30);
            }
            draw_text(out.layer, font, format!("{}.", i + 1).as_bytes(), 6, y, c1);
            let name = match editing {
                Some((_, e)) => format!("{}_", e.text),
                None => s.name.clone(),
            };
            draw_text(out.layer, font, name.as_bytes(), 24, y, c2);
            draw_text(out.layer, font, s.completed.to_string().as_bytes(), 210, y, c1);
            let score = s.score.to_string();
            let w = text_width(font, score.as_bytes());
            draw_text(out.layer, font, score.as_bytes(), 300 - w, y, c2);
        }
        out.palette = pal;
    }
}

// ---------------------------------------------------------------- victory

/// `VictoryScreen` (2390:2877): pic 7 by FizzleFade over 35 frames, held up
/// to 350 tics (a key ends it), then a 35-step fade out.
#[derive(Default)]
pub struct VictoryPic {
    t: u32,
    fading: Option<u32>,
}

impl Screen for VictoryPic {
    fn name(&self) -> &'static str {
        "victory"
    }

    fn tick(&mut self, ctx: &mut Ctx) -> Transition {
        self.t += 1;
        if let Some(f) = &mut self.fading {
            *f += 1;
            return if *f >= 35 { Transition::Pop } else { Transition::None };
        }
        if self.t >= 70 + 350 || (self.t > 70 && any_key(ctx.input)) {
            self.fading = Some(0);
        }
        Transition::None
    }

    fn draw(&mut self, a: &Assets, out: &mut Frame) {
        out.frame.clear(0);
        out.layer.clear(0);
        out.layer_key = None;
        if let Some(p) = a.pics.get(pic::VICTORY) {
            let shown = (self.t / 2).min(35);
            for y in 0..200 {
                for x in 0..320 {
                    if fizzle(x, y) % 35 < shown {
                        out.layer.pixels[y * 320 + x] = p.pixels[y * 320 + x];
                    }
                }
            }
        }
        out.palette = match self.fading {
            Some(f) => faded(&a.picpal, f as i32, 35, [0, 0, 0]),
            None => a.picpal,
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
    fn name(&self) -> &'static str {
        "quit"
    }

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
    Interpolate,
    Textures,
    Floors,
    XbrzMenus,
    XbrzHud,
    Controls,
    AlwaysRun,
    Mouse,
    PadTurn,
    Rumble,
    Master,
    Sfx,
    Music,
    Channels,
    Effects,
}

const OPTS: [Opt; 21] = [
    Opt::Display,
    Opt::Scale,
    Opt::Aspect,
    Opt::Fullscreen,
    Opt::Smooth,
    Opt::IntegerScale,
    Opt::Interpolate,
    Opt::Textures,
    Opt::Floors,
    Opt::XbrzMenus,
    Opt::XbrzHud,
    Opt::Controls,
    Opt::AlwaysRun,
    Opt::Mouse,
    Opt::PadTurn,
    Opt::Rumble,
    Opt::Master,
    Opt::Sfx,
    Opt::Music,
    Opt::Channels,
    Opt::Effects,
];

/// The port's settings: Classic or Enhanced display, resolution, aspect,
/// controls and audio. Left/Right change a value; Esc returns.
pub struct Options {
    cur: usize,
    shown: crate::settings::Settings,
}

impl Options {
    pub fn new(audio_first: bool) -> Self {
        let audio = OPTS.iter().position(|&o| o == Opt::Master).unwrap_or(0);
        Self { cur: if audio_first { audio } else { 0 }, shown: Default::default() }
    }

    fn label(o: Opt) -> &'static str {
        match o {
            Opt::Display => "DISPLAY",
            Opt::Scale => "RESOLUTION",
            Opt::Aspect => "ASPECT",
            Opt::Fullscreen => "FULLSCREEN",
            Opt::Smooth => "SMOOTH FILTER",
            Opt::IntegerScale => "INTEGER SCALING",
            Opt::Interpolate => "SMOOTH MOTION",
            Opt::Textures => "TEXTURES",
            Opt::Floors => "FLOORS",
            Opt::XbrzMenus => "XBRZ MENUS",
            Opt::XbrzHud => "XBRZ STATUS BAR",
            Opt::Controls => "CONTROLS",
            Opt::AlwaysRun => "ALWAYS RUN",
            Opt::Mouse => "MOUSE SPEED",
            Opt::PadTurn => "PAD TURN SPEED",
            Opt::Rumble => "RUMBLE",
            Opt::Master => "MASTER VOLUME",
            Opt::Sfx => "SOUND FX VOLUME",
            Opt::Music => "MUSIC VOLUME",
            Opt::Channels => "SOUND VOICES",
            Opt::Effects => "SOUND EFFECTS",
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
            Opt::Interpolate => on(s.video.interpolate),
            Opt::Textures => match s.video.xbrz {
                n @ 2..=6 => format!("XBRZ {n}X"),
                _ => "ORIGINAL".into(),
            },
            Opt::Floors => if s.video.smooth_floors { "SMOOTH" } else { "ORIGINAL" }.into(),
            Opt::XbrzMenus => on(s.video.xbrz_menus),
            Opt::XbrzHud => on(s.video.xbrz_hud),
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
            Opt::Effects => match s.audio.effects.as_str() {
                "adlib" => "ADLIB".into(),
                "pc" => "PC SPEAKER".into(),
                _ => "DIGITIZED".into(),
            },
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
            Opt::Interpolate => s.video.interpolate = !s.video.interpolate,
            Opt::Textures => {
                let all = [0usize, 2, 3, 4, 5, 6];
                let cur = all.iter().position(|&n| n == s.video.xbrz).unwrap_or(0) as i32;
                s.video.xbrz = all[(cur + d).rem_euclid(all.len() as i32) as usize];
            }
            Opt::Floors => s.video.smooth_floors = !s.video.smooth_floors,
            Opt::XbrzMenus => s.video.xbrz_menus = !s.video.xbrz_menus,
            Opt::XbrzHud => s.video.xbrz_hud = !s.video.xbrz_hud,
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
            Opt::Effects => {
                let all = ["digitized", "adlib", "pc"];
                let cur = all.iter().position(|e| *e == s.audio.effects).unwrap_or(0) as i32;
                s.audio.effects = all[(cur + d).rem_euclid(3) as usize].into();
            }
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
    fn name(&self) -> &'static str {
        "options"
    }

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
        // Twelve rows fit; the list scrolls with the cursor.
        const ROWS: usize = 12;
        let first = self.cur.saturating_sub(ROWS - 1).min(OPTS.len() - ROWS);
        if first > 0 {
            draw_text(out.layer, font, b"^", 304, 24, dim);
        }
        if first + ROWS < OPTS.len() {
            draw_text(out.layer, font, b"v", 304, 180, dim);
        }
        for (i, &o) in OPTS.iter().enumerate().skip(first).take(ROWS) {
            let y = 24 + (i - first) as i32 * 14;
            let c = if i == self.cur { hi } else { fg };
            draw_text(out.layer, font, Self::label(o).as_bytes(), 20, y, c);
            let v = Self::value(o, &self.shown);
            draw_text(out.layer, font, v.as_bytes(), 180, y, if i == self.cur { hi } else { dim });
        }
        out.palette = pal;
    }
}
