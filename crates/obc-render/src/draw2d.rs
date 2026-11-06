//! 2D drawing: pictures, proportional text, bars (`WOLFSRC/ID_VH.C`).

use obc_formats::vgagraph::{Font, Pic};

use crate::framebuffer::Framebuffer;

/// `VWB_DrawPic`: opaque copy at (`x`, `y`). The original rounds `x` down to a
/// multiple of 4 (planar VGA); callers pass already-aligned coordinates.
pub fn draw_pic(fb: &mut Framebuffer, pic: &Pic, x: i32, y: i32) {
    let w = pic.width as usize;
    for (row, line) in pic.pixels.chunks_exact(w).enumerate() {
        for (col, &p) in line.iter().enumerate() {
            fb.put(x + col as i32, y + row as i32, p);
        }
    }
}

/// Draws a picture skipping pixels of `transparent` color.
pub fn draw_pic_masked(fb: &mut Framebuffer, pic: &Pic, x: i32, y: i32, transparent: u8) {
    let w = pic.width as usize;
    for (row, line) in pic.pixels.chunks_exact(w).enumerate() {
        for (col, &p) in line.iter().enumerate() {
            if p != transparent {
                fb.put(x + col as i32, y + row as i32, p);
            }
        }
    }
}

/// `VW_DrawPropString`: draws `text` with `color` where glyph bytes are nonzero.
/// Returns the x after the last character.
pub fn draw_text(fb: &mut Framebuffer, font: &Font, text: &[u8], x: i32, y: i32, color: u8) -> i32 {
    let mut px = x;
    for &c in text {
        let w = font.width(c) as i32;
        let glyph = font.glyph(c);
        for row in 0..font.height as i32 {
            for col in 0..w {
                if glyph[(row * w + col) as usize] != 0 {
                    fb.put(px + col, y + row, color);
                }
            }
        }
        px += w;
    }
    px
}

/// The second proportional-string drawer (`2e72:0569` → `2e72:0117`): OBC's
/// fonts are multi-coloured, and this one writes each glyph pixel's own colour
/// (nonzero bytes), not a single font colour. Returns the x after the text.
pub fn draw_text_native(fb: &mut Framebuffer, font: &Font, text: &[u8], x: i32, y: i32) -> i32 {
    let mut px = x;
    for &c in text {
        let w = font.width(c) as i32;
        let glyph = font.glyph(c);
        for row in 0..font.height as i32 {
            for col in 0..w {
                let v = glyph[(row * w + col) as usize];
                if v != 0 {
                    fb.put(px + col, y + row, v);
                }
            }
        }
        px += w;
    }
    px
}

/// Text with a one-pixel drop shadow (down-right).
pub fn draw_text_shadow(
    fb: &mut Framebuffer,
    font: &Font,
    text: &[u8],
    x: i32,
    y: i32,
    color: u8,
    shadow: u8,
) {
    draw_text(fb, font, text, x + 1, y + 1, shadow);
    draw_text(fb, font, text, x, y, color);
}

/// Width of `text` in pixels.
pub fn text_width(font: &Font, text: &[u8]) -> i32 {
    font.measure(text) as i32
}

/// Text centered horizontally on `cx`.
pub fn draw_text_centered(
    fb: &mut Framebuffer,
    font: &Font,
    text: &[u8],
    cx: i32,
    y: i32,
    color: u8,
) {
    let w = text_width(font, text);
    draw_text(fb, font, text, cx - w / 2, y, color);
}
