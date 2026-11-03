//! The status bar and in-view messages (docs/re/PLAYER.md §5).

use obc_render::Framebuffer;
use obc_render::draw2d::{draw_pic, draw_text};
use obc_sim::world::World;

use crate::assets::Assets;

/// VGAGRAPH picture indices (chunk − 3).
mod pic {
    pub const STATUSBAR: usize = 29;
    pub const BLANK_DIGIT: usize = 9;
    pub const DIGIT0: usize = 10;
    pub const EVENT_BLANK: usize = 36;
    pub const ALLY_DEATH: usize = 37;
    pub const SPARKLE: usize = 50;
}

/// Event-box animation (`30d6:0923`, `30d6:0999`, `30d6:09ea`), advanced per frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EventBox {
    #[default]
    Blank,
    /// Item icon shown for 24 frames.
    Item { pic: usize, frames: u8 },
    /// Ally-death animation: pics 37–42, three frames each.
    AllyDeath { step: u8 },
    /// Sparkle: pics 50–52, one frame each.
    Sparkle { step: u8 },
}

/// HUD state kept by the UI between frames.
#[derive(Debug, Clone, Default)]
pub struct Hud {
    pub event: EventBox,
    /// Current message and frames-left counter (`GameMessage`, 125 tics).
    pub message: Option<(String, i16)>,
}

/// Icon picture for a pickup item (`ShowItemIcon` 30d6:09ea).
pub fn item_pic(item: u16) -> Option<usize> {
    match item {
        3 => Some(47),
        4 => Some(44),
        8 => Some(45),
        9 => Some(46),
        14 => Some(48),
        _ => None,
    }
}

impl Hud {
    pub fn show_item(&mut self, item: u16) {
        if let Some(p) = item_pic(item) {
            self.event = EventBox::Item { pic: p, frames: 24 };
        }
    }

    pub fn ally_died(&mut self) {
        self.event = EventBox::AllyDeath { step: 0 };
    }

    pub fn sparkle(&mut self) {
        if self.event == EventBox::Blank {
            self.event = EventBox::Sparkle { step: 0 };
        }
    }

    pub fn message(&mut self, text: String) {
        self.message = Some((text, 125));
    }

    /// One original frame of `tics` tics.
    pub fn frame(&mut self, tics: i16) {
        self.event = match self.event {
            EventBox::Item { frames: 0, .. } => EventBox::Blank,
            EventBox::Item { pic, frames } => EventBox::Item { pic, frames: frames - 1 },
            EventBox::AllyDeath { step } if step >= 17 => EventBox::Blank,
            EventBox::AllyDeath { step } => EventBox::AllyDeath { step: step + 1 },
            EventBox::Sparkle { step } if step >= 3 => EventBox::Blank,
            EventBox::Sparkle { step } => EventBox::Sparkle { step: step + 1 },
            EventBox::Blank => EventBox::Blank,
        };
        if let Some((_, t)) = &mut self.message {
            *t -= tics;
            if *t <= 0 {
                self.message = None;
            }
        }
    }

    /// Draws the status bar into the 320×200 layer.
    pub fn draw(&self, fb: &mut Framebuffer, a: &Assets, w: &World) {
        let pics = &a.pics;
        let Some(bar) = pics.get(pic::STATUSBAR) else { return };
        draw_pic(fb, bar, 0, 160);
        let g = &w.gamestate;

        let event = match self.event {
            EventBox::Item { pic, .. } => pic,
            EventBox::AllyDeath { step } => pic::ALLY_DEATH + usize::from(step / 3).min(5),
            EventBox::Sparkle { step } if step < 3 => pic::SPARKLE + usize::from(step),
            _ => pic::EVENT_BLANK,
        };
        if let Some(p) = pics.get(event) {
            draw_pic(fb, p, 8, 168);
        }
        let weapon_pic = match g.chosenweapon {
            0 => 34,
            1 => 30,
            2 => 31,
            3 => 33,
            4 => 32,
            _ => 35,
        };
        if let Some(p) = pics.get(weapon_pic) {
            draw_pic(fb, p, 48, 168);
        }
        // Ally order letter (30d6:08c0): pic 25 for mode 0x1B, 26 mode 1, 27 mode 0,
        // 28 with no allies.
        let letter = match (w.allycount, w.allymode) {
            (..=0, _) => 28,
            (_, 0x1B) => 25,
            (_, 1) => 26,
            _ => 27,
        };
        if let Some(p) = pics.get(letter) {
            draw_pic(fb, p, 112, 170);
        }
        latch_number(fb, a, 128, 171, 1, i32::from(w.allycount.clamp(0, 9)));
        // Kill lamp: exit open at half the kills.
        let lamp = if g.killcount >= g.killtotal {
            0x0E
        } else if g.killcount >= g.killtotal / 2 {
            2
        } else {
            1
        };
        fb.bar(116, 188, 4, 6, lamp);
        let diff = if g.difficulty < 4 { i32::from(g.difficulty) + 1 } else { 5 };
        latch_number(fb, a, 128, 187, 1, diff);
        bar_segments(fb, 174, i32::from(g.health) / 19 + 1);
        let ammo = if g.weapon <= 0 { 19 } else { i32::from(g.clips[(g.weapon - 1) as usize % 4]) };
        bar_segments(fb, 182, ammo);
        bar_segments(fb, 190, if g.armor > 0 { i32::from(g.armor) / 19 + 1 } else { 0 });
        latch_number(fb, a, 256, 171, 7, g.score);
        latch_number(fb, a, 256, 187, 3, i32::from(g.killcount));
        latch_number(fb, a, 288, 187, 3, i32::from(g.mapon) + 1);
    }

    /// Draws the current message at view-relative (5,5) with a shadow at (6,6).
    pub fn draw_message(&self, fb: &mut Framebuffer, a: &Assets, view_x: i32, view_y: i32) {
        let (Some((text, _)), Some(font)) = (&self.message, a.fonts.first()) else { return };
        draw_text(fb, font, text.as_bytes(), view_x + 6, view_y + 6, 0);
        draw_text(fb, font, text.as_bytes(), view_x + 5, view_y + 5, 0x0F);
    }
}

/// `LatchNumber`: right-aligned digits in `width` 8-pixel cells, blank padded.
fn latch_number(fb: &mut Framebuffer, a: &Assets, x: i32, y: i32, width: usize, value: i32) {
    let s = value.max(0).to_string();
    let s = if s.len() > width { &s[s.len() - width..] } else { &s };
    let pad = width - s.len();
    for i in 0..width {
        let p = if i < pad {
            pic::BLANK_DIGIT
        } else {
            pic::DIGIT0 + usize::from(s.as_bytes()[i - pad] - b'0')
        };
        if let Some(pic) = a.pics.get(p) {
            draw_pic(fb, pic, x + 8 * i as i32, y);
        }
    }
}

/// 19 segments of 3×4 at x = 171 + 4i (`DrawHealth` etc.).
fn bar_segments(fb: &mut Framebuffer, y: i32, lit: i32) {
    let lit = lit.clamp(0, 19);
    let color = if lit > 14 {
        0x0E
    } else if lit >= 7 {
        0x02
    } else {
        0x01
    };
    for i in 0..19 {
        fb.bar(171 + 4 * i, y, 3, 4, if i < lit { color } else { 0 });
    }
}
