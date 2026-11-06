//! Palettes and indexed-to-RGBA conversion.

/// 256 RGB colors, 8 bits per channel.
pub type Palette = [[u8; 3]; 256];

/// Expands an indexed image to RGBA through `palette`.
pub fn to_rgba(pixels: &[u8], palette: &Palette, out: &mut Vec<u8>) {
    out.resize(pixels.len() * 4, 255);
    for (dst, &p) in out.chunks_exact_mut(4).zip(pixels) {
        let c = palette[p as usize];
        dst[0] = c[0];
        dst[1] = c[1];
        dst[2] = c[2];
        dst[3] = 255;
    }
}

/// `palette` scaled toward black by `level` of `steps` (VL_FadeOut/FadeIn with
/// 6-bit precision: each channel is computed on the 0..63 DAC scale, then expanded).
pub fn faded(palette: &Palette, level: i32, steps: i32, target: [u8; 3]) -> Palette {
    let mut out = *palette;
    if steps <= 0 {
        return out;
    }
    for (o, c) in out.iter_mut().zip(palette) {
        for ch in 0..3 {
            let orig = i32::from(c[ch] >> 2);
            let tgt = i32::from(target[ch] >> 2);
            let v = orig + (tgt - orig) * level / steps;
            o[ch] = expand6(v.clamp(0, 63) as u8);
        }
    }
    out
}

/// Expands a 6-bit VGA DAC value to 8 bits the way DOSBox does.
pub fn expand6(v: u8) -> u8 {
    let v = v & 0x3F;
    (v << 2) | (v >> 4)
}
