/// An 8-bit indexed image, row-major.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Framebuffer {
    pub width: usize,
    pub height: usize,
    pub pixels: Vec<u8>,
}

impl Framebuffer {
    pub fn new(width: usize, height: usize) -> Self {
        Self { width, height, pixels: vec![0; width * height] }
    }

    /// Resizes (contents become undefined) if the size differs.
    pub fn resize(&mut self, width: usize, height: usize) {
        if (self.width, self.height) != (width, height) {
            *self = Self::new(width, height);
        }
    }

    pub fn clear(&mut self, color: u8) {
        self.pixels.fill(color);
    }

    #[inline]
    pub fn put(&mut self, x: i32, y: i32, color: u8) {
        if x >= 0 && y >= 0 && (x as usize) < self.width && (y as usize) < self.height {
            self.pixels[y as usize * self.width + x as usize] = color;
        }
    }

    #[inline]
    pub fn get(&self, x: usize, y: usize) -> u8 {
        self.pixels[y * self.width + x]
    }

    /// Filled rectangle clipped to the buffer (`VWB_Bar`).
    pub fn bar(&mut self, x: i32, y: i32, w: i32, h: i32, color: u8) {
        let x0 = x.max(0) as usize;
        let y0 = y.max(0) as usize;
        let x1 = ((x + w).max(0) as usize).min(self.width);
        let y1 = ((y + h).max(0) as usize).min(self.height);
        for row in y0..y1 {
            if x0 < x1 {
                self.pixels[row * self.width + x0..row * self.width + x1].fill(color);
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
}
