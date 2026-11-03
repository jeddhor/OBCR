//! Presentation geometry: Classic 320×200, and Enhanced high-resolution and
//! widescreen frames.
//!
//! Enhanced frames are `200·k` pixels tall and keep the original 1.2:1 pixel
//! aspect, so the original projection math applies unchanged at k× size; the
//! 320×200 2D layer is upscaled k× into the centered 4:3 region.

use obc_render::Framebuffer;

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
                let row = (y * k + dy) * dst.width + ox + x * k;
                dst.pixels[row..row + k].fill(c);
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
    fn widescreen_keeps_height_and_widens() {
        let p = Presentation { scale: 4, aspect: Aspect::Wide16x9 };
        let (w, h) = p.frame_size();
        assert_eq!(h, 800);
        assert!(w > 1280 && w % 4 == 0);
        assert_eq!(p.inset_x(), (w - 1280) / 2);
    }
}
