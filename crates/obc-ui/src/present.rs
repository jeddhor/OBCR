//! Presentation geometry: Classic 320×200, and Enhanced high-resolution and
//! widescreen frames.
//!
//! Enhanced frames are `200·k` pixels tall and keep the original 1.2:1 pixel
//! aspect, so the original projection math applies unchanged at k× size; the
//! 320×200 2D layer is upscaled k× into the centered 4:3 region.

use obc_render::Framebuffer;
use obc_render::palette::Palette;
use obc_render::xbrz::{self, Image, Metric, Mix};

/// Display aspect ratio of the whole frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Aspect {
    /// 4:3, the original.
    Standard,
    Wide16x10,
    Wide16x9,
    Wide21x9,
}

impl Aspect {
    pub fn ratio(self) -> f32 {
        match self {
            Aspect::Standard => 4.0 / 3.0,
            Aspect::Wide16x10 => 16.0 / 10.0,
            Aspect::Wide16x9 => 16.0 / 9.0,
            Aspect::Wide21x9 => 64.0 / 27.0,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Aspect::Standard => "4:3",
            Aspect::Wide16x10 => "16:10",
            Aspect::Wide16x9 => "16:9",
            Aspect::Wide21x9 => "21:9",
        }
    }

    pub fn next(self) -> Self {
        match self {
            Aspect::Standard => Aspect::Wide16x10,
            Aspect::Wide16x10 => Aspect::Wide16x9,
            Aspect::Wide16x9 => Aspect::Wide21x9,
            Aspect::Wide21x9 => Aspect::Standard,
        }
    }
}

/// How frames are laid out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Presentation {
    /// Resolution multiple of the original (1 = 320×200).
    pub scale: usize,
    pub aspect: Aspect,
}

impl Presentation {
    pub const CLASSIC: Self = Self { scale: 1, aspect: Aspect::Standard };

    pub fn is_classic(self) -> bool {
        self == Self::CLASSIC
    }

    /// Frame size in pixels. Width is a multiple of 4.
    pub fn frame_size(self) -> (usize, usize) {
        let h = 200 * self.scale;
        // Pixels are 1.2 times taller than wide, as on the original display.
        let w = (h as f32 * self.aspect.ratio() * 1.2).round() as usize;
        let w = (w.max(320 * self.scale) + 3) & !3;
        (w, h)
    }

    /// X offset of the 4:3 (320·k wide) region inside the frame.
    pub fn inset_x(self) -> usize {
        (self.frame_size().0 - 320 * self.scale) / 2
    }
}

/// Upscales a 320×200 indexed layer into `dst` at `p.scale`, centered,
/// skipping pixels equal to `transparent` when given.
pub fn blit_layer(
    dst: &mut Framebuffer,
    layer: &Framebuffer,
    p: Presentation,
    transparent: Option<u8>,
) {
    let k = p.scale;
    let ox = p.inset_x();
    for y in 0..layer.height.min(dst.height / k) {
        for x in 0..layer.width {
            let c = layer.pixels[y * layer.width + x];
            if transparent == Some(c) {
                continue;
            }
            for dy in 0..k {
                dst.fill_run(ox + x * k, y * k + dy, k, c);
            }
        }
    }
}

/// Enhanced XBRZ MENUS / XBRZ HUD: the 2D layer upscaled with xBRZ at the
/// presentation scale (at most 6×, then nearest to the frame), cached while
/// the layer, its key and the palette stay the same.
#[derive(Default)]
pub struct LayerScaler {
    cached: Option<LayerKey>,
    out: Vec<Mix>,
    metric: Option<(Palette, Option<u8>, Metric)>,
}

#[derive(PartialEq)]
struct LayerKey {
    pixels: Vec<u8>,
    width: usize,
    transparent: Option<u8>,
    palette: Palette,
    factor: usize,
}

impl LayerScaler {
    /// Like [`blit_layer`]; `palette` is the one the frame will be shown with.
    pub fn blit(
        &mut self,
        dst: &mut Framebuffer,
        layer: &Framebuffer,
        p: Presentation,
        transparent: Option<u8>,
        palette: &Palette,
    ) {
        let k = p.scale;
        if k < 2 {
            blit_layer(dst, layer, p, transparent);
            return;
        }
        let f = k.min(6);
        let key = LayerKey {
            pixels: layer.pixels.clone(),
            width: layer.width,
            transparent,
            palette: *palette,
            factor: f,
        };
        if self.cached.as_ref() != Some(&key) {
            if self.metric.as_ref().is_none_or(|(pal, t, _)| pal != palette || *t != transparent) {
                self.metric = Some((*palette, transparent, Metric::new(palette, transparent)));
            }
            let Some((_, _, metric)) = &self.metric else { return };
            let img = Image {
                px: &layer.pixels,
                w: layer.width,
                h: layer.height,
                wrap_x: false,
                wrap_y: false,
            };
            let threads = std::thread::available_parallelism().map_or(4, |n| n.get()).clamp(1, 8);
            self.out = xbrz::scale(&img, f, metric, threads);
            self.cached = Some(key);
        }
        let (sw, ox, fw) = (layer.width * f, p.inset_x(), dst.width);
        let (cols, rows) =
            ((layer.width * k).min(fw.saturating_sub(ox)), (layer.height * k).min(dst.height));
        let (pixels, mix) = dst.planes_mut();
        for dy in 0..rows {
            let src = &self.out[dy * f / k * sw..][..sw];
            let base = dy * fw + ox;
            let (px, mx) = (&mut pixels[base..base + cols], &mut mix[base..base + cols]);
            for (dx, (p, q)) in px.iter_mut().zip(mx.iter_mut()).enumerate() {
                let m = if k == f { src[dx] } else { src[dx * f / k] };
                if m.w == 0 && transparent == Some(m.a) {
                    continue;
                }
                *p = m.a;
                *q = [m.b, m.w];
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classic_is_original_size() {
        assert_eq!(Presentation::CLASSIC.frame_size(), (320, 200));
        assert_eq!(Presentation::CLASSIC.inset_x(), 0);
    }

    #[test]
    fn xbrz_layer_keeps_transparency_and_flat_areas() {
        let mut pal = [[0u8; 3]; 256];
        for (i, c) in pal.iter_mut().enumerate() {
            *c = [i as u8; 3];
        }
        let mut layer = Framebuffer::new(320, 200);
        layer.clear(0xFF);
        layer.bar(100, 100, 50, 20, 40);
        let p = Presentation { scale: 3, aspect: Aspect::Standard };
        let mut dst = Framebuffer::new(960, 600);
        dst.clear(7);
        let mut scaler = LayerScaler::default();
        scaler.blit(&mut dst, &layer, p, Some(0xFF), &pal);
        assert_eq!(dst.get(10, 10), 7, "transparent layer pixels keep the frame");
        assert_eq!(dst.get(110 * 3, 105 * 3), 40, "inside the bar");
        // The cached result is reused and identical.
        let first = dst.clone();
        scaler.blit(&mut dst, &layer, p, Some(0xFF), &pal);
        assert_eq!(first, dst);
    }

    #[test]
    fn widescreen_keeps_height_and_widens() {
        let p = Presentation { scale: 4, aspect: Aspect::Wide16x9 };
        let (w, h) = p.frame_size();
        assert_eq!(h, 800);
        assert!(w > 1280 && w % 4 == 0);
        assert_eq!(p.inset_x(), (w - 1280) / 2);
    }
}
