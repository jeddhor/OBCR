use crate::palette::Palette;

/// An 8-bit indexed image, row-major.
///
/// Enhanced rendering can also blend two palette indices in one pixel
/// (xBRZ texels, smooth shading): the second index and its weight live in a
/// separate plane that is allocated on the first blended write and only
/// counts while [`Framebuffer::is_mixed`] is true. Plain writes clear the
/// pixel's weight, so 2D drawn over a blended 3D view is never tinted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Framebuffer {
    pub width: usize,
    pub height: usize,
    pub pixels: Vec<u8>,
    /// Second index and its weight (0–255) per pixel.
    mix: Vec<[u8; 2]>,
    mixed: bool,
}

impl Framebuffer {
    pub fn new(width: usize, height: usize) -> Self {
        Self { width, height, pixels: vec![0; width * height], mix: Vec::new(), mixed: false }
    }

    /// Resizes (contents become undefined) if the size differs.
    pub fn resize(&mut self, width: usize, height: usize) {
        if (self.width, self.height) != (width, height) {
            *self = Self::new(width, height);
        }
    }

    /// Starts a new frame: blends from the last frame no longer count. Every
    /// pixel must be redrawn after this.
    pub fn begin_frame(&mut self) {
        self.mixed = false;
    }

    /// Whether any blended pixel was written since the last clear.
    pub fn is_mixed(&self) -> bool {
        self.mixed
    }

    pub fn clear(&mut self, color: u8) {
        self.pixels.fill(color);
        self.mixed = false;
    }

    #[inline]
    pub fn put(&mut self, x: i32, y: i32, color: u8) {
        if x >= 0 && y >= 0 && (x as usize) < self.width && (y as usize) < self.height {
            let i = y as usize * self.width + x as usize;
            self.pixels[i] = color;
            if self.mixed {
                self.mix[i][1] = 0;
            }
        }
    }

    /// Index `a` blended with `b` at weight `w`/255.
    #[inline]
    pub fn put_mix(&mut self, x: i32, y: i32, a: u8, b: u8, w: u8) {
        if w == 0 {
            self.put(x, y, a);
            return;
        }
        if x >= 0 && y >= 0 && (x as usize) < self.width && (y as usize) < self.height {
            if !self.mixed {
                self.start_mixing();
            }
            let i = y as usize * self.width + x as usize;
            self.pixels[i] = a;
            self.mix[i] = [b, w];
        }
    }

    /// Both planes for direct writes of blended pixels (index, then
    /// `[second index, weight]`), row-major with stride `width`. Starts
    /// blending; a plain pixel must be written with weight 0.
    pub fn planes_mut(&mut self) -> (&mut [u8], &mut [[u8; 2]]) {
        if !self.mixed {
            self.start_mixing();
        }
        (&mut self.pixels, &mut self.mix)
    }

    fn start_mixing(&mut self) {
        if self.mix.len() == self.pixels.len() {
            self.mix.iter_mut().for_each(|m| m[1] = 0);
        } else {
            self.mix = vec![[0, 0]; self.pixels.len()];
        }
        self.mixed = true;
    }

    #[inline]
    pub fn get(&self, x: usize, y: usize) -> u8 {
        self.pixels[y * self.width + x]
    }

    /// The pixel as (index, second index, weight).
    pub fn get_mix(&self, x: usize, y: usize) -> (u8, u8, u8) {
        let i = y * self.width + x;
        if self.mixed && self.mix[i][1] != 0 {
            (self.pixels[i], self.mix[i][0], self.mix[i][1])
        } else {
            (self.pixels[i], self.pixels[i], 0)
        }
    }

    /// `n` pixels of `color` from (`x`, `y`) rightwards, unclipped: the run
    /// must lie inside the buffer.
    pub fn fill_run(&mut self, x: usize, y: usize, n: usize, color: u8) {
        let i = y * self.width + x;
        self.pixels[i..i + n].fill(color);
        if self.mixed {
            self.mix[i..i + n].iter_mut().for_each(|m| m[1] = 0);
        }
    }

    /// Filled rectangle clipped to the buffer (`VWB_Bar`).
    pub fn bar(&mut self, x: i32, y: i32, w: i32, h: i32, color: u8) {
        let x0 = x.max(0) as usize;
        let y0 = y.max(0) as usize;
        let x1 = ((x + w).max(0) as usize).min(self.width);
        let y1 = ((y + h).max(0) as usize).min(self.height);
        for row in y0..y1 {
            if x0 < x1 {
                self.fill_run(x0, row, x1 - x0, color);
            }
        }
    }

    /// Copies `src` into this buffer at (`x`, `y`), clipped.
    pub fn blit(&mut self, src: &Framebuffer, x: i32, y: i32) {
        for sy in 0..src.height {
            let dy = y + sy as i32;
            if dy < 0 || dy as usize >= self.height {
                continue;
            }
            for sx in 0..src.width {
                self.put(x + sx as i32, dy, src.pixels[sy * src.width + sx]);
            }
        }
    }

    /// Moves the image up by `rows`; the rows uncovered at the bottom become 0.
    pub fn scroll_up(&mut self, rows: usize) {
        let rows = rows.min(self.height);
        if rows == 0 {
            return;
        }
        let n = rows * self.width;
        self.pixels.copy_within(n.., 0);
        let len = self.pixels.len();
        self.pixels[len - n..].fill(0);
        if self.mixed {
            self.mix.copy_within(n.., 0);
            self.mix[len - n..].iter_mut().for_each(|m| m[1] = 0);
        }
    }

    /// Expands to RGBA through `palette`, blending mixed pixels.
    pub fn to_rgba(&self, palette: &Palette, out: &mut Vec<u8>) {
        if !self.mixed {
            crate::palette::to_rgba(&self.pixels, palette, out);
            return;
        }
        out.resize(self.pixels.len() * 4, 255);
        for ((dst, &p), m) in out.chunks_exact_mut(4).zip(&self.pixels).zip(&self.mix) {
            let a = palette[usize::from(p)];
            let c = if m[1] == 0 {
                a
            } else {
                let b = palette[usize::from(m[0])];
                let w = u32::from(m[1]);
                let lerp = |x: u8, y: u8| {
                    ((u32::from(x) * (255 - w) + u32::from(y) * w + 127) / 255) as u8
                };
                [lerp(a[0], b[0]), lerp(a[1], b[1]), lerp(a[2], b[2])]
            };
            dst[..3].copy_from_slice(&c);
            dst[3] = 255;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_writes_clear_blends() {
        let mut fb = Framebuffer::new(4, 2);
        fb.put_mix(1, 0, 10, 20, 128);
        assert!(fb.is_mixed());
        assert_eq!(fb.get_mix(1, 0), (10, 20, 128));
        fb.put(1, 0, 7);
        assert_eq!(fb.get_mix(1, 0), (7, 7, 0));
        fb.put_mix(2, 1, 1, 2, 3);
        fb.bar(0, 1, 4, 1, 5);
        assert_eq!(fb.get_mix(2, 1), (5, 5, 0));
        // A new frame forgets the old blends.
        fb.put_mix(3, 0, 1, 2, 200);
        fb.begin_frame();
        assert_eq!(fb.get_mix(3, 0), (1, 1, 0));
        fb.put_mix(0, 0, 4, 5, 6);
        assert_eq!(fb.get_mix(3, 0), (1, 1, 0), "stale weight cleared");
    }

    #[test]
    fn rgba_blends_two_indices() {
        let mut pal = [[0u8; 3]; 256];
        pal[1] = [0, 0, 0];
        pal[2] = [255, 100, 10];
        let mut fb = Framebuffer::new(2, 1);
        fb.put(0, 0, 2);
        fb.put_mix(1, 0, 1, 2, 255);
        let mut out = Vec::new();
        fb.to_rgba(&pal, &mut out);
        assert_eq!(&out[..4], &[255, 100, 10, 255]);
        assert_eq!(&out[4..], &[255, 100, 10, 255]);
        fb.put_mix(1, 0, 1, 2, 51);
        fb.to_rgba(&pal, &mut out);
        assert_eq!(&out[4..], &[51, 20, 2, 255]);
    }

    #[test]
    fn scrolling_moves_blends_too() {
        let mut fb = Framebuffer::new(2, 3);
        fb.put_mix(0, 2, 9, 8, 100);
        fb.scroll_up(1);
        assert_eq!(fb.get_mix(0, 1), (9, 8, 100));
        assert_eq!(fb.get_mix(0, 2), (0, 0, 0));
    }
}
