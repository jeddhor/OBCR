//! KEY BINDINGS: every action with its keyboard/mouse and gamepad inputs,
//! and rebinding (issue #2). Opened from OPTIONS and from the help pages.
//!
//! Up/Down choose an action, Left/Right the column, Enter (pad A) waits for
//! the new input, Backspace/Delete (pad X) clears the column, Esc (pad B)
//! returns. The last row resets everything to the scheme's defaults.

use obc_core::input::{InputState, pad, sc};
use obc_render::draw2d::{draw_text, text_width};

use crate::assets::Assets;
use crate::bindings::{ACTIONS, Action, Bind, Bindings, label};
use crate::menu::{MOVE_SOUND, nearest, play_digi};
use crate::screen::{Ctx, Frame, Screen, Transition};

/// Visible rows.
const ROWS: usize = 10;
/// Tics before waiting for an input gives up (5 s).
const CAPTURE_TICS: u32 = 350;
/// Tics a note stays on screen.
const NOTE_TICS: u32 = 210;

pub struct Rebind {
    row: usize,
    /// 0 keyboard/mouse, 1 gamepad.
    col: usize,
    /// Waiting for an input: tics left. Starts on the tic after the press,
    /// so the key that started it is not taken as the new binding.
    capture: Option<u32>,
    armed: bool,
    note: Option<(String, u32)>,
    shown: Bindings,
    scheme: String,
}

impl Default for Rebind {
    fn default() -> Self {
        Self::new()
    }
}

impl Rebind {
    pub fn new() -> Self {
        Self {
            row: 0,
            col: 0,
            capture: None,
            armed: false,
            note: None,
            shown: Bindings::default(),
            scheme: String::new(),
        }
    }

    fn rows() -> usize {
        ACTIONS.len() + 1
    }

    fn action(&self) -> Option<Action> {
        ACTIONS.get(self.row).map(|(a, _)| *a)
    }
}

fn pressed_any(input: &InputState, keys: &[u8], pads: u32) -> bool {
    keys.iter().any(|&k| input.pressed(k)) || input.pad_pressed(pads)
}

impl Screen for Rebind {
    fn name(&self) -> &'static str {
        "key bindings"
    }

    fn tick(&mut self, ctx: &mut Ctx) -> Transition {
        let input = ctx.input;
        ctx.requests.relative_mouse = Some(false);
        if let Some((_, t)) = &mut self.note {
            *t = t.saturating_sub(1);
            if *t == 0 {
                self.note = None;
            }
        }
        if let Some(left) = self.capture {
            let pad_col = self.col == 1;
            let cancel = input.pressed(sc::ESCAPE) || input.pad_pressed(pad::START) || left == 0;
            if cancel {
                self.capture = None;
            } else if !self.armed {
                // The press that started the wait.
                self.armed = true;
                self.capture = Some(left - 1);
            } else if let (Some(b), Some(a)) = (Bind::captured(input, pad_col), self.action()) {
                let taken = ctx.settings.input.bindings.bind(a, b);
                if !taken.is_empty() {
                    let from: Vec<&str> = taken.iter().map(|&t| label(t)).collect();
                    self.note =
                        Some((format!("{} MOVED FROM {}", b.name(), from.join(", ")), NOTE_TICS));
                }
                self.capture = None;
            } else {
                self.capture = Some(left - 1);
            }
            self.shown = ctx.settings.input.bindings.clone();
            self.scheme = ctx.settings.input.profile.clone();
            return Transition::None;
        }
        if pressed_any(input, &[sc::ESCAPE], pad::EAST | pad::BACK) {
            return Transition::Pop;
        }
        let n = Self::rows();
        if pressed_any(input, &[sc::UP], pad::DPAD_UP) {
            self.row = (self.row + n - 1) % n;
            play_digi(ctx, MOVE_SOUND);
        }
        if pressed_any(input, &[sc::DOWN], pad::DPAD_DOWN) {
            self.row = (self.row + 1) % n;
            play_digi(ctx, MOVE_SOUND);
        }
        if pressed_any(input, &[sc::PGUP], 0) {
            self.row = self.row.saturating_sub(ROWS);
        }
        if pressed_any(input, &[sc::PGDN], 0) {
            self.row = (self.row + ROWS).min(n - 1);
        }
        if pressed_any(input, &[sc::LEFT, sc::RIGHT, sc::TAB], pad::DPAD_LEFT | pad::DPAD_RIGHT) {
            self.col ^= 1;
        }
        let activate = pressed_any(input, &[sc::ENTER, sc::SPACE], pad::SOUTH);
        match self.action() {
            Some(a) => {
                if activate {
                    self.capture = Some(CAPTURE_TICS);
                    self.armed = false;
                } else if pressed_any(input, &[sc::BACKSPACE, sc::DELETE], pad::WEST) {
                    ctx.settings.input.bindings.clear(a, self.col == 1);
                }
            }
            None => {
                if activate {
                    let scheme = ctx.settings.input.profile.clone();
                    ctx.settings.input.bindings = Bindings::defaults(&scheme);
                    self.note = Some(("DEFAULTS RESTORED".into(), NOTE_TICS));
                }
            }
        }
        self.shown = ctx.settings.input.bindings.clone();
        self.scheme = ctx.settings.input.profile.clone();
        Transition::None
    }

    fn draw(&mut self, a: &Assets, out: &mut Frame) {
        out.frame.clear(0);
        out.layer.clear(0);
        out.layer_key = None;
        let pal = a.gamepal;
        let (fg, hi, dim, sel) = (
            nearest(&pal, [255, 255, 255]),
            nearest(&pal, [255, 220, 0]),
            nearest(&pal, [150, 150, 150]),
            nearest(&pal, [40, 40, 110]),
        );
        out.layer.bar(4, 2, 312, 196, nearest(&pal, [16, 16, 56]));
        let Some(font) = a.fonts.first() else { return };
        let title = b"KEY BINDINGS";
        draw_text(out.layer, font, title, 160 - text_width(font, title) / 2, 3, hi);
        // One device at a time: the font is too wide for both columns.
        let pad_view = self.col == 1;
        let head: &[u8] = if pad_view { b"<  GAMEPAD  >" } else { b"<  KEYBOARD AND MOUSE  >" };
        draw_text(out.layer, font, head, 160 - text_width(font, head) / 2, 17, dim);
        let vx =
            ACTIONS.iter().map(|(_, l)| text_width(font, l.as_bytes())).max().unwrap_or(120) + 18;
        let (right, line) = (308, 13);
        let n = Self::rows();
        let first = self.row.saturating_sub(ROWS - 1).min(n - ROWS);
        // Fits `s` into `w` pixels, ending in ".." when cut.
        let fit = |s: String, w: i32| -> Vec<u8> {
            let mut b = s.into_bytes();
            if text_width(font, &b) <= w {
                return b;
            }
            while !b.is_empty() && text_width(font, &b) + text_width(font, b"..") > w {
                b.pop();
            }
            b.extend_from_slice(b"..");
            b
        };
        for (i, r) in (first..(first + ROWS).min(n)).enumerate() {
            let y = 32 + i as i32 * line;
            let current = r == self.row;
            match ACTIONS.get(r) {
                Some((act, name)) => {
                    draw_text(
                        out.layer,
                        font,
                        name.as_bytes(),
                        10,
                        y,
                        if current { hi } else { fg },
                    );
                    if current {
                        out.layer.bar(vx - 3, y - 1, right - vx + 4, line, sel);
                    }
                    let text = match (current, self.capture.is_some()) {
                        (true, true) if pad_view => "PRESS A BUTTON".to_string(),
                        (true, true) => "PRESS A KEY".to_string(),
                        _ => self.shown.describe(*act, pad_view),
                    };
                    draw_text(
                        out.layer,
                        font,
                        &fit(text, right - vx),
                        vx,
                        y,
                        if current { hi } else { dim },
                    );
                }
                None => {
                    let t = format!("RESET TO {} DEFAULTS", self.scheme.to_uppercase());
                    draw_text(out.layer, font, t.as_bytes(), 10, y, if current { hi } else { fg });
                }
            }
        }
        if first > 0 {
            draw_text(out.layer, font, b"^", 300, 17, dim);
        }
        if first + ROWS < n {
            draw_text(out.layer, font, b"v", 300, 32 + (ROWS as i32 - 1) * line, dim);
        }
        let footer: String = if let Some((n, _)) = &self.note {
            n.clone()
        } else if self.capture.is_some() {
            "ESC OR START CANCELS".into()
        } else if let Some(act) = self.action() {
            // The whole list, which the table may have cut.
            self.shown.describe(act, pad_view)
        } else {
            String::new()
        };
        draw_text(out.layer, font, &fit(footer, 300), 10, 167, fg);
        let help: &[u8] =
            if pad_view { b"A BIND   X CLEAR   B BACK" } else { b"ENTER BIND  DEL CLEAR" };
        draw_text(out.layer, font, help, 160 - text_width(font, help) / 2, 183, dim);
        out.palette = pal;
    }
}
