//! PNG and WAV writers for exports.

use std::fs::File;
use std::io::BufWriter;
use std::path::Path;

use anyhow::{Context, Result};

fn create(path: &Path) -> Result<BufWriter<File>> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    Ok(BufWriter::new(File::create(path).with_context(|| path.display().to_string())?))
}

pub fn png_rgba(path: &Path, width: u32, height: u32, rgba: &[u8]) -> Result<()> {
    let mut enc = png::Encoder::new(create(path)?, width, height);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    enc.write_header()?.write_image_data(rgba)?;
    Ok(())
}

/// Lays out equally sized RGBA tiles in a grid with a 1-pixel gutter.
pub fn sheet(path: &Path, tiles: &[Vec<u8>], size: u32, columns: u32) -> Result<()> {
    let rows = (tiles.len() as u32).div_ceil(columns).max(1);
    let (w, h) = (columns * (size + 1), rows * (size + 1));
    let mut rgba = vec![0u8; (w * h * 4) as usize];
    for (i, tile) in tiles.iter().enumerate() {
        let (tx, ty) = ((i as u32 % columns) * (size + 1), (i as u32 / columns) * (size + 1));
        for y in 0..size {
            let src = &tile[(y * size * 4) as usize..((y + 1) * size * 4) as usize];
            let dst = (((ty + y) * w + tx) * 4) as usize;
            rgba[dst..dst + src.len()].copy_from_slice(src);
        }
    }
    png_rgba(path, w, h, &rgba)
}

/// Writes 8-bit unsigned mono samples.
pub fn wav_u8(path: &Path, rate: u32, samples: &[u8]) -> Result<()> {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: rate,
        bits_per_sample: 8,
        sample_format: hound::SampleFormat::Int,
    };
    let mut w = hound::WavWriter::new(create(path)?, spec)?;
    for &s in samples {
        w.write_sample((i16::from(s) - 128) as i8)?;
    }
    w.finalize()?;
    Ok(())
}

/// Renders PC speaker data (one PIT-divisor byte per 1/140 s, divisor = byte × 60)
/// as an 8-bit square wave.
pub fn pc_speaker(data: &[u8], rate: u32) -> Vec<u8> {
    const PIT_HZ: f64 = 1_193_181.0;
    let per_tick = rate as usize / 140;
    let mut out = Vec::with_capacity(data.len() * per_tick);
    let mut phase = 0.0f64;
    for &b in data {
        let freq = if b == 0 { 0.0 } else { PIT_HZ / (f64::from(b) * 60.0) };
        for _ in 0..per_tick {
            if freq == 0.0 {
                out.push(128);
                continue;
            }
            phase = (phase + freq / f64::from(rate)).fract();
            out.push(if phase < 0.5 { 128 + 40 } else { 128 - 40 });
        }
    }
    out
}
