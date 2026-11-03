//! Rendered sequences: `PlaySequence(name)` (ghidra 32a2:0bcb, GAMEFLOW §6).
//!
//! `F2`–`F6` are FLC files named `.BC`. A missing file is skipped silently,
//! as the original does. One 70 Hz tic counts as 16 "ms", so a frame lasts
//! `ceil(speed / 16)` tics; playback ends after frame `frames − 2` or on Esc,
//! then fades out over 30 steps.

use std::sync::Arc;

use obc_audio::{AudioCommand, Pan};
use obc_core::input::{pad, sc};
use obc_formats::flc::{Flic, decode};
use obc_render::palette::{Palette, faded};

use crate::assets::Assets;
use crate::screen::{Ctx, Frame, Screen, Transition};
use crate::sound::DIGI_RATE;

/// Per-frame sound cues (32a2:09fa): (frame, sound id).
fn cues(name: &str) -> &'static [(usize, u16)] {
    match name {
        "F1" => &[(0, 88), (128, 89), (185, 88), (258, 90), (319, 91)],
        "F2" => &[(1, 93)],
        "F3" => &[(1, 94)],
        "F5" => &[(75, 95), (130, 96)],
        "F6" => &[(1, 92)],
        _ => &[],
    }
}

const FADE_STEPS: u32 = 30;

pub struct Sequence {
    name: String,
    flic: Flic,
    frame: usize,
    wait: u32,
    delay: u32,
    fading: Option<u32>,
}

impl Sequence {
    /// Loads `name` (e.g. "F2"); `None` if the file is missing or unreadable.
    pub fn load(assets: &Assets, name: &str) -> Option<Self> {
        let bytes = assets.data.read(&format!("{name}.BC")).ok()?;
        let flic = decode(&bytes).ok()?;
        if flic.frames.is_empty() {
            return None;
        }
        let delay = flic.speed_ms.div_ceil(16).max(1);
        Some(Self { name: name.to_string(), flic, frame: 0, wait: 0, delay, fading: None })
    }

    fn cue(&self, ctx: &Ctx) {
        for &(f, id) in cues(&self.name) {
            if f == self.frame
                && let Some(d) = ctx.assets.digi.get(usize::from(id)).filter(|d| !d.is_empty())
            {
                ctx.audio.send(AudioCommand::PlayDigi {
                    channel: 0,
                    samples: Arc::from(d.as_slice()),
                    rate: DIGI_RATE,
                    pan: Pan::default(),
                });
            }
        }
    }
}

impl Screen for Sequence {
    fn tick(&mut self, ctx: &mut Ctx) -> Transition {
        if let Some(step) = &mut self.fading {
            *step += 1;
            return if *step >= FADE_STEPS { Transition::Pop } else { Transition::None };
        }
        let skip = ctx.input.down(sc::ESCAPE) || ctx.input.pad_pressed(pad::START | pad::EAST);
        if self.wait == 0 {
            self.cue(ctx);
        }
        self.wait += 1;
        if self.wait >= self.delay {
            self.wait = 0;
            self.frame += 1;
        }
        if skip || self.frame + 1 >= self.flic.frames.len() {
            self.frame = self.frame.min(self.flic.frames.len() - 1);
            self.fading = Some(0);
        }
        Transition::None
    }

    fn draw(&mut self, _assets: &Assets, out: &mut Frame) {
        let f = &self.flic.frames[self.frame.min(self.flic.frames.len() - 1)];
        out.frame.clear(0);
        let (w, h) =
            (usize::from(self.flic.width).min(320), usize::from(self.flic.height).min(200));
        out.layer.clear(0);
        for y in 0..h {
            let src = &f.pixels[y * usize::from(self.flic.width)..][..w];
            out.layer.pixels[y * 320..y * 320 + w].copy_from_slice(src);
        }
        out.layer_key = None;
        let pal: Palette = f.palette;
        out.palette = match self.fading {
            Some(step) => faded(&pal, step as i32, FADE_STEPS as i32, [0, 0, 0]),
            None => pal,
        };
    }
}
