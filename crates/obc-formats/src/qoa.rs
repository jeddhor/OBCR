//! QOA, the "Quite OK Audio" format (<https://qoaformat.org>, MIT-licensed
//! reference by Dominic Szablewski), for the CD music OBCR rips from the
//! user's disc image (docs/DECISIONS.md, ADR-022).
//!
//! Lossy at 3.2 bits per sample, decodes with integer arithmetic only, and
//! is small enough to implement here instead of adding a dependency. The
//! encoder and decoder follow the specification, so the files also play in
//! other QOA tools.
//!
//! Layout (all big-endian): an 8-byte file header `"qoaf"` + samples per
//! channel, then frames of up to 256 slices × 20 samples per channel. Each
//! frame header gives the channel count, rate, sample count and byte size,
//! followed by every channel's LMS state (history and weights, 4 × `i16`
//! each); each slice is one `u64`: a 4-bit scale factor and 20 3-bit
//! residuals.

use crate::error::{FormatError, Result};

const MAGIC: u32 = 0x716f_6166; // "qoaf"
const SLICE_LEN: usize = 20;
const SLICES_PER_FRAME: usize = 256;
/// Samples per channel in a full frame.
pub const FRAME_LEN: usize = SLICE_LEN * SLICES_PER_FRAME;
const LMS_LEN: usize = 4;
/// Most channels a frame may carry.
pub const MAX_CHANNELS: usize = 8;

const SCALEFACTOR: [i32; 16] =
    [1, 7, 21, 45, 84, 138, 211, 304, 421, 562, 731, 928, 1157, 1419, 1715, 2048];

/// `round(scalefactor × {0.75, −0.75, 2.5, −2.5, 4.5, −4.5, 7, −7})`, rounding
/// halves away from zero (the reference's `qoa_dequant_tab`).
const DEQUANT: [[i32; 8]; 16] = {
    const Q4: [i32; 8] = [3, -3, 10, -10, 18, -18, 28, -28];
    let mut t = [[0; 8]; 16];
    let mut s = 0;
    while s < 16 {
        let mut q = 0;
        while q < 8 {
            let m = (SCALEFACTOR[s] * Q4[q].abs() + 2) / 4;
            t[s][q] = if Q4[q] < 0 { -m } else { m };
            q += 1;
        }
        s += 1;
    }
    t
};

/// Quantized residual (−8..=8, offset by 8) to its 3-bit code.
const QUANT: [u8; 17] = [7, 7, 7, 5, 5, 3, 3, 1, 0, 0, 2, 2, 4, 4, 6, 6, 6];

const RECIPROCAL: [i32; 16] = {
    let mut t = [0; 16];
    let mut s = 0;
    while s < 16 {
        t[s] = ((1 << 16) + SCALEFACTOR[s] - 1) / SCALEFACTOR[s];
        s += 1;
    }
    t
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Lms {
    history: [i32; LMS_LEN],
    weights: [i32; LMS_LEN],
}

impl Lms {
    fn predict(&self) -> i32 {
        // `int` arithmetic in the reference; wrapping keeps bit-exactness.
        let p = self
            .history
            .iter()
            .zip(&self.weights)
            .fold(0i32, |a, (h, w)| a.wrapping_add(h.wrapping_mul(*w)));
        p >> 13
    }

    fn update(&mut self, sample: i32, residual: i32) {
        let delta = residual >> 4;
        for (w, h) in self.weights.iter_mut().zip(&self.history) {
            *w = w.wrapping_add(if *h < 0 { -delta } else { delta });
        }
        self.history.rotate_left(1);
        self.history[LMS_LEN - 1] = sample;
    }
}

fn clamp_s16(v: i32) -> i32 {
    v.clamp(-32768, 32767)
}

/// `qoa_div`: `v / scalefactor`, rounded away from zero.
fn div(v: i32, sf: usize) -> i32 {
    let n = ((i64::from(v) * i64::from(RECIPROCAL[sf]) + (1 << 15)) >> 16) as i32;
    n + (v.signum() - n.signum())
}

fn be_u16(b: &[u8], at: usize) -> u16 {
    u16::from_be_bytes([b[at], b[at + 1]])
}

fn be_u64(b: &[u8], at: usize) -> u64 {
    let mut a = [0; 8];
    a.copy_from_slice(&b[at..at + 8]);
    u64::from_be_bytes(a)
}

fn invalid(msg: impl Into<String>) -> FormatError {
    FormatError::Invalid { what: "QOA", msg: msg.into() }
}

/// Encodes interleaved 16-bit `samples` (`channels` per frame) as a QOA file.
pub fn encode(samples: &[i16], channels: usize, rate: u32) -> Result<Vec<u8>> {
    let mut enc = Encoder::new(channels, rate)?;
    let mut out = Vec::with_capacity(samples.len() * 2 / 5 + 64);
    enc.header((samples.len() / channels) as u32, &mut out);
    for frame in samples.chunks(FRAME_LEN * channels) {
        enc.frame(frame, &mut out);
    }
    Ok(out)
}

/// A streaming encoder: write [`Encoder::header`] once (the total sample
/// count must be known), then [`Encoder::frame`] for each block of up to
/// [`FRAME_LEN`] samples per channel; only the last block may be shorter.
#[derive(Debug)]
pub struct Encoder {
    channels: usize,
    rate: u32,
    lms: Vec<Lms>,
    prev_sf: Vec<usize>,
}

impl Encoder {
    pub fn new(channels: usize, rate: u32) -> Result<Self> {
        if !(1..=MAX_CHANNELS).contains(&channels) || rate == 0 || rate > 0xFF_FFFF {
            return Err(invalid(format!("unsupported layout: {channels} channels at {rate} Hz")));
        }
        let lms = Lms { history: [0; 4], weights: [0, 0, -(1 << 13), 1 << 14] };
        Ok(Self { channels, rate, lms: vec![lms; channels], prev_sf: vec![0; channels] })
    }

    pub fn header(&self, samples_per_channel: u32, out: &mut Vec<u8>) {
        out.extend_from_slice(&MAGIC.to_be_bytes());
        out.extend_from_slice(&samples_per_channel.to_be_bytes());
    }

    /// Encodes one frame of interleaved samples.
    pub fn frame(&mut self, samples: &[i16], out: &mut Vec<u8>) {
        let ch = self.channels;
        let len = (samples.len() / ch).min(FRAME_LEN);
        let slices = len.div_ceil(SLICE_LEN);
        let size = 8 + LMS_LEN * 4 * ch + 8 * slices * ch;
        out.push(ch as u8);
        out.extend_from_slice(&self.rate.to_be_bytes()[1..]);
        out.extend_from_slice(&(len as u16).to_be_bytes());
        out.extend_from_slice(&(size as u16).to_be_bytes());
        for lms in &self.lms {
            let pack = |v: &[i32; 4]| v.iter().fold(0u64, |a, &x| (a << 16) | u64::from(x as u16));
            out.extend_from_slice(&pack(&lms.history).to_be_bytes());
            out.extend_from_slice(&pack(&lms.weights).to_be_bytes());
        }
        for start in (0..len).step_by(SLICE_LEN) {
            let slice_len = SLICE_LEN.min(len - start);
            for c in 0..ch {
                let mut best: Option<(u64, u64, Lms, usize)> = None; // (error, slice, lms, sf)
                for sfi in 0..16 {
                    let sf = (sfi + self.prev_sf[c]) % 16;
                    let mut lms = self.lms[c];
                    let mut slice = sf as u64;
                    let mut err: u64 = 0;
                    let mut over = false;
                    for si in start..start + slice_len {
                        let sample = i32::from(samples[si * ch + c]);
                        let predicted = lms.predict();
                        let scaled = div(sample.wrapping_sub(predicted), sf).clamp(-8, 8);
                        let q = QUANT[(scaled + 8) as usize];
                        let dq = DEQUANT[sf][usize::from(q)];
                        let rec = clamp_s16(predicted.saturating_add(dq));
                        // Penalise runaway weights (reference encoder, v1.0).
                        let wp: i64 =
                            lms.weights.iter().map(|&w| i64::from(w) * i64::from(w)).sum();
                        let penalty = ((wp >> 18) - 0x8ff).max(0) as u64;
                        let e = i64::from(sample - rec);
                        err += (e * e) as u64 + penalty * penalty;
                        if best.as_ref().is_some_and(|b| err > b.0) {
                            over = true;
                            break;
                        }
                        lms.update(rec, dq);
                        slice = (slice << 3) | u64::from(q);
                    }
                    if !over && best.as_ref().is_none_or(|b| err < b.0) {
                        best = Some((err, slice, lms, sf));
                    }
                }
                let (_, slice, lms, sf) = best.expect("scale factor 0 always completes");
                self.prev_sf[c] = sf;
                self.lms[c] = lms;
                let slice = slice << ((SLICE_LEN - slice_len) * 3);
                out.extend_from_slice(&slice.to_be_bytes());
            }
        }
    }
}

/// A QOA file's header values.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Info {
    pub channels: usize,
    pub rate: u32,
    /// Samples per channel.
    pub samples: u32,
}

/// Decodes a QOA file frame by frame, for streaming playback.
#[derive(Debug, Clone)]
pub struct Decoder<B> {
    data: B,
    pos: usize,
    info: Info,
}

impl<B: AsRef<[u8]>> Decoder<B> {
    pub fn new(data: B) -> Result<Self> {
        let b = data.as_ref();
        if b.len() < 16 || be_u64(b, 0) >> 32 != u64::from(MAGIC) {
            return Err(invalid("not a QOA file"));
        }
        let samples = (be_u64(b, 0) & 0xFFFF_FFFF) as u32;
        let channels = usize::from(b[8]);
        let rate = u32::from_be_bytes([0, b[9], b[10], b[11]]);
        if channels == 0 || channels > MAX_CHANNELS || rate == 0 {
            return Err(invalid("bad first frame header"));
        }
        Ok(Self { data, pos: 8, info: Info { channels, rate, samples } })
    }

    pub fn info(&self) -> Info {
        self.info
    }

    /// Starts again from the first frame.
    pub fn rewind(&mut self) {
        self.pos = 8;
    }

    /// Decodes the next frame into `out` (interleaved; cleared first).
    /// Returns `false` at the end of the file or on a damaged frame.
    pub fn next_frame(&mut self, out: &mut Vec<i16>) -> bool {
        out.clear();
        let b = self.data.as_ref();
        let p = self.pos;
        if p + 8 > b.len() {
            return false;
        }
        let ch = usize::from(b[p]);
        let len = usize::from(be_u16(b, p + 4));
        let size = usize::from(be_u16(b, p + 6));
        let slices = len.div_ceil(SLICE_LEN);
        if ch != self.info.channels
            || len > FRAME_LEN
            || size != 8 + LMS_LEN * 4 * ch + 8 * slices * ch
            || p + size > b.len()
        {
            return false;
        }
        let mut lms = [Lms { history: [0; 4], weights: [0; 4] }; MAX_CHANNELS];
        let mut at = p + 8;
        for l in lms.iter_mut().take(ch) {
            let (h, w) = (be_u64(b, at), be_u64(b, at + 8));
            for i in 0..LMS_LEN {
                l.history[i] = i32::from((h >> (48 - 16 * i)) as u16 as i16);
                l.weights[i] = i32::from((w >> (48 - 16 * i)) as u16 as i16);
            }
            at += 16;
        }
        out.resize(len * ch, 0);
        for start in (0..len).step_by(SLICE_LEN) {
            for (c, l) in lms.iter_mut().enumerate().take(ch) {
                let mut slice = be_u64(b, at);
                at += 8;
                let sf = (slice >> 60) as usize;
                slice <<= 4;
                for si in start..(start + SLICE_LEN).min(len) {
                    let predicted = l.predict();
                    let q = ((slice >> 61) & 7) as usize;
                    let dq = DEQUANT[sf][q];
                    let rec = clamp_s16(predicted.saturating_add(dq));
                    out[si * ch + c] = rec as i16;
                    slice <<= 3;
                    l.update(rec, dq);
                }
            }
        }
        self.pos = p + size;
        true
    }
}

/// Decodes a whole file to interleaved samples.
pub fn decode(data: &[u8]) -> Result<(Info, Vec<i16>)> {
    let mut d = Decoder::new(data)?;
    let info = d.info();
    let mut all = Vec::with_capacity(info.samples as usize * info.channels);
    let mut frame = Vec::new();
    while d.next_frame(&mut frame) {
        all.extend_from_slice(&frame);
    }
    if all.len() != info.samples as usize * info.channels {
        return Err(invalid(format!(
            "{} samples decoded, header says {}",
            all.len() / info.channels,
            info.samples
        )));
    }
    Ok((info, all))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tone(n: usize) -> Vec<i16> {
        // Two sines and a slow envelope, like music more than a square wave.
        (0..n)
            .flat_map(|i| {
                let t = i as f64 / 44_100.0;
                let env = 0.3 + 0.2 * (t * 3.0).sin();
                let l = env
                    * ((t * 440.0 * std::f64::consts::TAU).sin()
                        + 0.5 * (t * 1250.0 * std::f64::consts::TAU).sin());
                let r = env * (t * 330.0 * std::f64::consts::TAU).sin();
                [(l * 20000.0) as i16, (r * 20000.0) as i16]
            })
            .collect()
    }

    #[test]
    fn tables_match_the_reference() {
        assert_eq!(DEQUANT[1], [5, -5, 18, -18, 32, -32, 49, -49]);
        assert_eq!(DEQUANT[2], [16, -16, 53, -53, 95, -95, 147, -147]);
        assert_eq!(DEQUANT[15], [1536, -1536, 5120, -5120, 9216, -9216, 14336, -14336]);
        assert_eq!(RECIPROCAL[..4], [65536, 9363, 3121, 1457]);
        assert_eq!(RECIPROCAL[15], 32);
    }

    #[test]
    fn round_trips_with_small_error() {
        let pcm = tone(FRAME_LEN * 2 + 777);
        let qoa = encode(&pcm, 2, 44_100).unwrap();
        assert_eq!(qoa.len(), 8 + 2 * (8 + 32 + 8 * 256 * 2) + (8 + 32 + 8 * 39 * 2));
        let (info, out) = decode(&qoa).unwrap();
        assert_eq!(info, Info { channels: 2, rate: 44_100, samples: (FRAME_LEN * 2 + 777) as u32 });
        let signal: f64 = pcm.iter().map(|&s| f64::from(s).powi(2)).sum();
        let noise: f64 =
            pcm.iter().zip(&out).map(|(&a, &b)| (f64::from(a) - f64::from(b)).powi(2)).sum();
        let snr_db = 10.0 * (signal / noise).log10();
        assert!(snr_db > 30.0, "SNR {snr_db:.1} dB");
    }

    #[test]
    fn rejects_garbage() {
        assert!(Decoder::new(&b"RIFF0000WAVEfmt "[..]).is_err());
        let mut qoa = encode(&tone(100), 2, 22_050).unwrap();
        qoa.truncate(qoa.len() - 3);
        assert!(decode(&qoa).is_err());
    }
}
