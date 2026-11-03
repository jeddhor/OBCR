//! Autodesk FLC/FLI animations: the rendered sequences `F2.BC`, `F3.BC`,
//! `F4.BC`, `F6.BC` (docs/re/DATA_FORMATS.md §1.1). Decodes every frame to a
//! full indexed image plus palette.

use crate::bytes::{slice, u8_at, u16_at, u32_at};
use crate::error::{Result, invalid};

const MAGIC_FLC: u16 = 0xAF12;
const MAGIC_FLI: u16 = 0xAF11;
const FRAME_CHUNK: u16 = 0xF1FA;
const PREFIX_CHUNK: u16 = 0xF100;

/// A decoded animation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Flic {
    pub width: u16,
    pub height: u16,
    /// Delay between frames in milliseconds.
    pub speed_ms: u32,
    pub frames: Vec<FlicFrame>,
}

/// One displayed frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlicFrame {
    pub pixels: Vec<u8>,
    /// 256 RGB entries, 8 bits per channel.
    pub palette: [[u8; 3]; 256],
}

/// Decodes an FLC or FLI file.
pub fn decode(data: &[u8]) -> Result<Flic> {
    const WHAT: &str = "FLC header";
    let magic = u16_at(data, 4, WHAT)?;
    if magic != MAGIC_FLC && magic != MAGIC_FLI {
        return Err(invalid(WHAT, format!("bad magic {magic:#06x}")));
    }
    let frames = u16_at(data, 6, WHAT)? as usize;
    let width = u16_at(data, 8, WHAT)?;
    let height = u16_at(data, 10, WHAT)?;
    if width == 0 || height == 0 || width > 1280 || height > 1024 {
        return Err(invalid(WHAT, format!("bad size {width}x{height}")));
    }
    let speed = u32_at(data, 16, WHAT)?;
    // FLI speed is in 1/70 s units; FLC speed is in milliseconds.
    let speed_ms = if magic == MAGIC_FLI {
        (u64::from(speed) * 1000 / 70).min(u32::MAX.into()) as u32
    } else {
        speed
    };
    let first = if magic == MAGIC_FLC { u32_at(data, 80, WHAT)? as usize } else { 128 };
    let first = if first == 0 { 128 } else { first };

    let mut pixels = vec![0u8; width as usize * height as usize];
    let mut palette = [[0u8; 3]; 256];
    let mut out = Vec::with_capacity(frames);
    let mut pos = first;
    while out.len() < frames && pos < data.len() {
        let size = u32_at(data, pos, "FLC chunk")? as usize;
        let kind = u16_at(data, pos + 4, "FLC chunk")?;
        if size < 6 {
            return Err(invalid("FLC chunk", format!("chunk of size {size} at {pos}")));
        }
        let chunk = slice(data, pos, size, "FLC chunk")?;
        match kind {
            FRAME_CHUNK => {
                decode_frame(chunk, width as usize, height as usize, &mut pixels, &mut palette)?;
                out.push(FlicFrame { pixels: pixels.clone(), palette });
            }
            PREFIX_CHUNK => {}
            _ => {
                return Err(invalid("FLC chunk", format!("unexpected chunk {kind:#06x} at {pos}")));
            }
        }
        pos += size;
    }
    Ok(Flic { width, height, speed_ms, frames: out })
}

fn decode_frame(
    frame: &[u8],
    w: usize,
    h: usize,
    pixels: &mut [u8],
    palette: &mut [[u8; 3]; 256],
) -> Result<()> {
    let subchunks = u16_at(frame, 6, "FLC frame")? as usize;
    let mut pos = 16;
    for _ in 0..subchunks {
        let size = u32_at(frame, pos, "FLC subchunk")? as usize;
        let kind = u16_at(frame, pos + 4, "FLC subchunk")?;
        if size < 6 {
            return Err(invalid("FLC subchunk", "size below header"));
        }
        let body = slice(frame, pos + 6, size - 6, "FLC subchunk")?;
        match kind {
            4 => color(body, palette, false)?,
            11 => color(body, palette, true)?,
            7 => delta_flc(body, w, h, pixels)?,
            12 => delta_fli(body, w, h, pixels)?,
            13 => pixels.fill(0),
            15 => byte_run(body, w, h, pixels)?,
            16 => pixels.copy_from_slice(slice(body, 0, w * h, "FLC literal")?),
            _ => {} // PSTAMP (18) and unknown chunks carry nothing we display.
        }
        pos += size;
    }
    Ok(())
}

fn color(body: &[u8], palette: &mut [[u8; 3]; 256], six_bit: bool) -> Result<()> {
    const WHAT: &str = "FLC color";
    let packets = u16_at(body, 0, WHAT)?;
    let mut pos = 2;
    let mut index = 0usize;
    for _ in 0..packets {
        index += u8_at(body, pos, WHAT)? as usize;
        let count = match u8_at(body, pos + 1, WHAT)? {
            0 => 256,
            n => n as usize,
        };
        pos += 2;
        for _ in 0..count {
            let rgb = slice(body, pos, 3, WHAT)?;
            if index < 256 {
                palette[index] = if six_bit {
                    [expand6(rgb[0]), expand6(rgb[1]), expand6(rgb[2])]
                } else {
                    [rgb[0], rgb[1], rgb[2]]
                };
            }
            index += 1;
            pos += 3;
        }
    }
    Ok(())
}

/// Expands a 6-bit VGA DAC value to 8 bits.
pub fn expand6(v: u8) -> u8 {
    let v = v & 0x3F;
    (v << 2) | (v >> 4)
}

fn byte_run(body: &[u8], w: usize, h: usize, pixels: &mut [u8]) -> Result<()> {
    const WHAT: &str = "FLC byte run";
    let mut pos = 0;
    for y in 0..h {
        pos += 1; // obsolete packet count
        let mut x = 0;
        while x < w {
            let count = u8_at(body, pos, WHAT)? as i8;
            pos += 1;
            if count >= 0 {
                let v = u8_at(body, pos, WHAT)?;
                pos += 1;
                for _ in 0..count {
                    put(pixels, w, x, y, v)?;
                    x += 1;
                }
            } else {
                for _ in 0..(-(count as i32)) {
                    put(pixels, w, x, y, u8_at(body, pos, WHAT)?)?;
                    pos += 1;
                    x += 1;
                }
            }
        }
    }
    Ok(())
}

fn delta_flc(body: &[u8], w: usize, h: usize, pixels: &mut [u8]) -> Result<()> {
    const WHAT: &str = "FLC delta";
    let lines = u16_at(body, 0, WHAT)? as usize;
    let mut pos = 2;
    let mut y = 0usize;
    let mut done = 0;
    while done < lines {
        let word = u16_at(body, pos, WHAT)?;
        pos += 2;
        match word >> 14 {
            0b11 => {
                y = y.wrapping_add((-(word as i16) as i32) as usize);
                continue;
            }
            0b10 => {
                if y < h {
                    put(pixels, w, w - 1, y, word as u8)?;
                }
                continue;
            }
            _ => {}
        }
        let packets = word;
        let mut x = 0usize;
        for _ in 0..packets {
            x += u8_at(body, pos, WHAT)? as usize;
            let count = u8_at(body, pos + 1, WHAT)? as i8;
            pos += 2;
            if count >= 0 {
                for _ in 0..count {
                    let pair = slice(body, pos, 2, WHAT)?;
                    put(pixels, w, x, y, pair[0])?;
                    put(pixels, w, x + 1, y, pair[1])?;
                    x += 2;
                    pos += 2;
                }
            } else {
                let pair = slice(body, pos, 2, WHAT)?;
                pos += 2;
                for _ in 0..(-(count as i32)) {
                    put(pixels, w, x, y, pair[0])?;
                    put(pixels, w, x + 1, y, pair[1])?;
                    x += 2;
                }
            }
        }
        y += 1;
        done += 1;
    }
    Ok(())
}

fn delta_fli(body: &[u8], w: usize, h: usize, pixels: &mut [u8]) -> Result<()> {
    const WHAT: &str = "FLI delta";
    let first = u16_at(body, 0, WHAT)? as usize;
    let lines = u16_at(body, 2, WHAT)? as usize;
    let mut pos = 4;
    for y in first..first + lines {
        let packets = u8_at(body, pos, WHAT)?;
        pos += 1;
        let mut x = 0usize;
        for _ in 0..packets {
            x += u8_at(body, pos, WHAT)? as usize;
            let count = u8_at(body, pos + 1, WHAT)? as i8;
            pos += 2;
            if count >= 0 {
                for _ in 0..count {
                    put(pixels, w, x, y.min(h), u8_at(body, pos, WHAT)?)?;
                    x += 1;
                    pos += 1;
                }
            } else {
                let v = u8_at(body, pos, WHAT)?;
                pos += 1;
                for _ in 0..(-(count as i32)) {
                    put(pixels, w, x, y.min(h), v)?;
                    x += 1;
                }
            }
        }
    }
    Ok(())
}

fn put(pixels: &mut [u8], w: usize, x: usize, y: usize, v: u8) -> Result<()> {
    if x >= w {
        return Err(invalid("FLC", "pixel outside frame"));
    }
    let p = pixels.get_mut(y * w + x).ok_or_else(|| invalid("FLC", "pixel outside frame"))?;
    *p = v;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn six_bit_expansion() {
        assert_eq!(expand6(0), 0);
        assert_eq!(expand6(63), 255);
        assert_eq!(expand6(32), 130);
    }

    proptest::proptest! {
        #[test]
        fn decoder_never_panics(mut data in proptest::collection::vec(proptest::num::u8::ANY, 0..400)) {
            if data.len() >= 12 {
                data[4] = 0x12; data[5] = 0xAF; data[8] = 8; data[9] = 0; data[10] = 4; data[11] = 0;
            }
            let _ = decode(&data);
        }
    }
}
