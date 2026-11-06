//! xBRZ upscaling (2×–6×) for palette images: Enhanced textures and screens.
//!
//! Derived from xBRZ 1.6, Copyright (C) Zenju, distributed under the GNU
//! General Public License version 3 (`dosbox-x/src/libs/xBRZ/xbrz.cpp`).
//! This file is a derivative work and is therefore distributed under the
//! GPL-3.0 as well (ADR-020).
//!
//! The rules, thresholds and blend tables are xBRZ's. What differs:
//!
//! - It works on palette indices. Colour distance and equality come from a
//!   [`Metric`] built from a palette (ITU-R BT.2020 YCbCr, exact rather than
//!   xBRZ's 64 MB lookup table), with one optional transparent index that
//!   behaves like alpha 0 (xBRZ's `ColorDistanceARGB`).
//! - Each output pixel is a [`Mix`] of at most two indices with a weight
//!   instead of an RGB value, so the renderer can still shade per index and
//!   the frame can still go through palette effects. A blend of three or more
//!   colours keeps the two heaviest; transparency is resolved at 50 %.
//! - The corner pre-pass runs over the whole image first (xBRZ streams it
//!   row by row), which allows wrap-around edges for tiling textures. With
//!   clamped edges the result is the same as xBRZ's single-stripe scan:
//!   corners left of column 0 and above row 0 are never blended.

use crate::palette::Palette;

/// One output pixel: palette index `a` blended with `b` at weight `w`/255.
/// With a transparent index `t`, a transparent pixel is `Mix::plain(t)`;
/// opaque pixels never contain `t`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Mix {
    pub a: u8,
    pub b: u8,
    pub w: u8,
}

impl Mix {
    pub const fn plain(c: u8) -> Self {
        Self { a: c, b: c, w: 0 }
    }
}

/// An upscaled 64×64 texture: `64·n` square, column-major like VSWAP pages.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HiTexture {
    pub n: usize,
    pub texels: Vec<Mix>,
}

impl HiTexture {
    /// Side length in texels.
    pub fn size(&self) -> usize {
        64 * self.n
    }

    /// Texel at fine column `col`, fine row `row`.
    #[inline]
    pub fn at(&self, col: usize, row: usize) -> Mix {
        self.texels[col * self.size() + row]
    }

    /// Upscales a column-major 64×64 page. `wrap_cols` joins the last column
    /// to the first (walls repeat sideways); `wrap_rows` joins the bottom row
    /// to the top (floors repeat both ways).
    pub fn from_page(
        page: &[u8],
        n: usize,
        metric: &Metric,
        wrap_cols: bool,
        wrap_rows: bool,
    ) -> Self {
        // Column-major is row-major for the transposed image: xBRZ's rows
        // are the page's columns.
        let img = Image { px: page, w: 64, h: 64, wrap_x: wrap_rows, wrap_y: wrap_cols };
        Self { n, texels: scale(&img, n, metric, 1) }
    }
}

/// xBRZ's `ScalerCfg` defaults.
const EQUAL_COLOR_TOLERANCE: f64 = 30.0;
const DOMINANT_DIRECTION_THRESHOLD: f64 = 3.6;
const STEEP_DIRECTION_THRESHOLD: f64 = 2.2;

/// Colour distance and equality between palette indices.
pub struct Metric {
    dist: Vec<f32>,
    /// Smallest index with the same colour (equality test), per index.
    canon: [u8; 256],
    transparent: Option<u8>,
}

impl Metric {
    pub fn new(pal: &Palette, transparent: Option<u8>) -> Self {
        let mut canon = [0u8; 256];
        for i in 0..256 {
            canon[i] = if Some(i as u8) == transparent {
                i as u8
            } else {
                (0..=i).find(|&j| Some(j as u8) != transparent && pal[j] == pal[i]).unwrap_or(i)
                    as u8
            };
        }
        let mut dist = vec![0f32; 256 * 256];
        for i in 0..256 {
            for j in 0..256 {
                let ai = if Some(i as u8) == transparent { 0.0 } else { 1.0 };
                let aj = if Some(j as u8) == transparent { 0.0 } else { 1.0 };
                let d = dist_ycbcr(pal[i], pal[j]);
                // ColorDistanceARGB: min(a1, a2)·d + 255·|a1 − a2|.
                let v =
                    if ai < aj { ai * d + 255.0 * (aj - ai) } else { aj * d + 255.0 * (ai - aj) };
                dist[i * 256 + j] = v as f32;
            }
        }
        Self { dist, canon, transparent }
    }

    #[inline]
    fn d(&self, a: u8, b: u8) -> f64 {
        f64::from(self.dist[usize::from(a) * 256 + usize::from(b)])
    }

    #[inline]
    fn eq(&self, a: u8, b: u8) -> bool {
        self.d(a, b) < EQUAL_COLOR_TOLERANCE
    }

    #[inline]
    fn is_transparent(&self, c: u8) -> bool {
        self.transparent == Some(c)
    }
}

/// `distYCbCr` with luminance weight 1 (BT.2020 coefficients).
fn dist_ycbcr(p1: [u8; 3], p2: [u8; 3]) -> f64 {
    let r = f64::from(p1[0]) - f64::from(p2[0]);
    let g = f64::from(p1[1]) - f64::from(p2[1]);
    let b = f64::from(p1[2]) - f64::from(p2[2]);
    const K_B: f64 = 0.0593;
    const K_R: f64 = 0.2627;
    const K_G: f64 = 1.0 - K_B - K_R;
    let scale_b = 0.5 / (1.0 - K_B);
    let scale_r = 0.5 / (1.0 - K_R);
    let y = K_R * r + K_G * g + K_B * b;
    let c_b = scale_b * (b - y);
    let c_r = scale_r * (r - y);
    (y * y + c_b * c_b + c_r * c_r).sqrt()
}

/// A palette image with edge handling.
pub struct Image<'a> {
    pub px: &'a [u8],
    pub w: usize,
    pub h: usize,
    pub wrap_x: bool,
    pub wrap_y: bool,
}

impl Image<'_> {
    #[inline]
    fn at(&self, x: isize, y: isize) -> u8 {
        let (w, h) = (self.w as isize, self.h as isize);
        let x = if self.wrap_x { x.rem_euclid(w) } else { x.clamp(0, w - 1) };
        let y = if self.wrap_y { y.rem_euclid(h) } else { y.clamp(0, h - 1) };
        self.px[y as usize * self.w + x as usize]
    }
}

const NONE: u8 = 0;
const NORMAL: u8 = 1;
const DOMINANT: u8 = 2;

/// `preProcessCorners` for the corner between F, G, J and K, with F at `at`
/// in the padded canonical image `c` (stride `pw`):
/// `blend_f | blend_g << 2 | blend_j << 4 | blend_k << 6`. Canonical indices
/// are equal exactly when the colours are.
fn pre_process(c: &[u8], pw: usize, m: &Metric, at: usize) -> u8 {
    let (up, dn, dn2) = (at - pw, at + pw, at + 2 * pw);
    // Kernel A–P; A, D, M and P do not take part.
    let (b, cc) = (c[up], c[up + 1]);
    let (e, f, g, h) = (c[at - 1], c[at], c[at + 1], c[at + 2]);
    let (i, j, k, l) = (c[dn - 1], c[dn], c[dn + 1], c[dn + 2]);
    let (n, o) = (c[dn2], c[dn2 + 1]);
    if (f == g && j == k) || (f == j && g == k) {
        return 0;
    }
    const WEIGHT: f64 = 4.0;
    let jg = m.d(i, f) + m.d(f, cc) + m.d(n, k) + m.d(k, h) + WEIGHT * m.d(j, g);
    let fk = m.d(e, j) + m.d(j, o) + m.d(b, g) + m.d(g, l) + WEIGHT * m.d(f, k);
    let (mut bf, mut bg, mut bj, mut bk) = (NONE, NONE, NONE, NONE);
    if jg < fk {
        let t = if DOMINANT_DIRECTION_THRESHOLD * jg < fk { DOMINANT } else { NORMAL };
        if f != g && f != j {
            bf = t;
        }
        if k != j && k != g {
            bk = t;
        }
    } else if fk < jg {
        let t = if DOMINANT_DIRECTION_THRESHOLD * fk < jg { DOMINANT } else { NORMAL };
        if j != f && j != k {
            bj = t;
        }
        if g != f && g != k {
            bg = t;
        }
    }
    bf | bg << 2 | bj << 4 | bk << 6
}

/// Blend accumulator for one output pixel: premultiplied weights per index.
#[derive(Clone, Copy)]
struct Acc {
    idx: [u8; 4],
    w: [f32; 4],
    n: usize,
}

impl Acc {
    const EMPTY: Self = Self { idx: [0; 4], w: [0.0; 4], n: 0 };

    fn solid(c: u8, m: &Metric) -> Self {
        let mut a = Self { idx: [0; 4], w: [0.0; 4], n: 0 };
        if !m.is_transparent(c) {
            a.idx[0] = c;
            a.w[0] = 1.0;
            a.n = 1;
        }
        a
    }

    /// `alphaGrad<num, den>`: front colour `c` at opacity num/den over this
    /// pixel (xBRZ's `gradientARGB`, premultiplied).
    fn grad(&mut self, num: u16, den: u16, c: u8, m: &Metric) {
        let f = f32::from(num) / f32::from(den);
        for w in &mut self.w[..self.n] {
            *w *= 1.0 - f;
        }
        if m.is_transparent(c) {
            return;
        }
        if let Some(k) = (0..self.n).find(|&k| self.idx[k] == c) {
            self.w[k] += f;
        } else if self.n < 4 {
            self.idx[self.n] = c;
            self.w[self.n] = f;
            self.n += 1;
        } else {
            let (k, _) = self
                .w
                .iter()
                .enumerate()
                .fold((0, f32::MAX), |best, (k, &w)| if w < best.1 { (k, w) } else { best });
            if f > self.w[k] {
                self.idx[k] = c;
                self.w[k] = f;
            }
        }
    }

    fn finish(&self, m: &Metric) -> Mix {
        let alpha: f32 = self.w[..self.n].iter().sum();
        if self.n == 0 || alpha < 0.5 {
            return Mix::plain(m.transparent.unwrap_or(0));
        }
        let (mut i1, mut i2) = (0usize, usize::MAX);
        for k in 1..self.n {
            if self.w[k] > self.w[i1] {
                i2 = i1;
                i1 = k;
            } else if i2 == usize::MAX || self.w[k] > self.w[i2] {
                i2 = k;
            }
        }
        if i2 == usize::MAX {
            return Mix::plain(self.idx[i1]);
        }
        let (w1, w2) = (self.w[i1], self.w[i2]);
        let w = (255.0 * w2 / (w1 + w2)).round() as u8;
        if w == 0 { Mix::plain(self.idx[i1]) } else { Mix { a: self.idx[i1], b: self.idx[i2], w } }
    }
}

/// One blend step of a scaler table: `G(i, j, m, n)` = `alphaGrad<m, n>` on
/// output (row i, column j); `S(i, j)` = assign.
#[derive(Clone, Copy)]
enum Op {
    G(u8, u8, u16, u16),
    S(u8, u8),
}
use Op::{G, S};

/// xBRZ's `Scaler2x`–`Scaler6x`, per factor:
/// `[shallow, steep, steep_and_shallow, diagonal, corner]`.
const OPS: [[&[Op]; 5]; 5] = [
    [
        &[G(1, 0, 1, 4), G(1, 1, 3, 4)],
        &[G(0, 1, 1, 4), G(1, 1, 3, 4)],
        &[G(1, 0, 1, 4), G(0, 1, 1, 4), G(1, 1, 5, 6)],
        &[G(1, 1, 1, 2)],
        &[G(1, 1, 21, 100)],
    ],
    [
        &[G(2, 0, 1, 4), G(1, 2, 1, 4), G(2, 1, 3, 4), S(2, 2)],
        &[G(0, 2, 1, 4), G(2, 1, 1, 4), G(1, 2, 3, 4), S(2, 2)],
        &[G(2, 0, 1, 4), G(0, 2, 1, 4), G(2, 1, 3, 4), G(1, 2, 3, 4), S(2, 2)],
        &[G(1, 2, 1, 8), G(2, 1, 1, 8), G(2, 2, 7, 8)],
        &[G(2, 2, 45, 100)],
    ],
    [
        &[G(3, 0, 1, 4), G(2, 2, 1, 4), G(3, 1, 3, 4), G(2, 3, 3, 4), S(3, 2), S(3, 3)],
        &[G(0, 3, 1, 4), G(2, 2, 1, 4), G(1, 3, 3, 4), G(3, 2, 3, 4), S(2, 3), S(3, 3)],
        &[
            G(3, 1, 3, 4),
            G(1, 3, 3, 4),
            G(3, 0, 1, 4),
            G(0, 3, 1, 4),
            G(2, 2, 1, 3),
            S(3, 3),
            S(3, 2),
            S(2, 3),
        ],
        &[G(3, 2, 1, 2), G(2, 3, 1, 2), S(3, 3)],
        &[G(3, 3, 68, 100), G(3, 2, 9, 100), G(2, 3, 9, 100)],
    ],
    [
        &[
            G(4, 0, 1, 4),
            G(3, 2, 1, 4),
            G(2, 4, 1, 4),
            G(4, 1, 3, 4),
            G(3, 3, 3, 4),
            S(4, 2),
            S(4, 3),
            S(4, 4),
            S(3, 4),
        ],
        &[
            G(0, 4, 1, 4),
            G(2, 3, 1, 4),
            G(4, 2, 1, 4),
            G(1, 4, 3, 4),
            G(3, 3, 3, 4),
            S(2, 4),
            S(3, 4),
            S(4, 4),
            S(4, 3),
        ],
        &[
            G(0, 4, 1, 4),
            G(2, 3, 1, 4),
            G(1, 4, 3, 4),
            G(4, 0, 1, 4),
            G(3, 2, 1, 4),
            G(4, 1, 3, 4),
            G(3, 3, 2, 3),
            S(2, 4),
            S(3, 4),
            S(4, 4),
            S(4, 2),
            S(4, 3),
        ],
        &[G(4, 2, 1, 8), G(3, 3, 1, 8), G(2, 4, 1, 8), G(4, 3, 7, 8), G(3, 4, 7, 8), S(4, 4)],
        &[G(4, 4, 86, 100), G(4, 3, 23, 100), G(3, 4, 23, 100)],
    ],
    [
        &[
            G(5, 0, 1, 4),
            G(4, 2, 1, 4),
            G(3, 4, 1, 4),
            G(5, 1, 3, 4),
            G(4, 3, 3, 4),
            G(3, 5, 3, 4),
            S(5, 2),
            S(5, 3),
            S(5, 4),
            S(5, 5),
            S(4, 4),
            S(4, 5),
        ],
        &[
            G(0, 5, 1, 4),
            G(2, 4, 1, 4),
            G(4, 3, 1, 4),
            G(1, 5, 3, 4),
            G(3, 4, 3, 4),
            G(5, 3, 3, 4),
            S(2, 5),
            S(3, 5),
            S(4, 5),
            S(5, 5),
            S(4, 4),
            S(5, 4),
        ],
        &[
            G(0, 5, 1, 4),
            G(2, 4, 1, 4),
            G(1, 5, 3, 4),
            G(3, 4, 3, 4),
            G(5, 0, 1, 4),
            G(4, 2, 1, 4),
            G(5, 1, 3, 4),
            G(4, 3, 3, 4),
            S(2, 5),
            S(3, 5),
            S(4, 5),
            S(5, 5),
            S(4, 4),
            S(5, 4),
            S(5, 2),
            S(5, 3),
        ],
        &[G(5, 3, 1, 2), G(4, 4, 1, 2), G(3, 5, 1, 2), S(4, 5), S(5, 5), S(5, 4)],
        &[G(5, 5, 97, 100), G(4, 5, 42, 100), G(5, 4, 42, 100), G(5, 3, 6, 100), G(3, 5, 6, 100)],
    ],
];

/// Kernel positions a–i read under each rotation (xBRZ's `get_x<rotDeg>`).
const ROT_KERNEL: [[usize; 9]; 4] = [
    [0, 1, 2, 3, 4, 5, 6, 7, 8],
    [6, 3, 0, 7, 4, 1, 8, 5, 2],
    [8, 7, 6, 5, 4, 3, 2, 1, 0],
    [2, 5, 8, 1, 4, 7, 0, 3, 6],
];

/// `rotateBlendInfo<rotDeg>`.
fn rotate_blend(b: u8, rot: usize) -> u8 {
    match rot {
        1 => b.rotate_left(2),
        2 => b.rotate_left(4),
        3 => b.rotate_left(6),
        _ => b,
    }
}

/// `OutputMatrix::ref<I, J>` under a rotation: (row, column) in the block.
fn rotate_ij(i: usize, j: usize, rot: usize, n: usize) -> (usize, usize) {
    let (mut i, mut j) = (i, j);
    for _ in 0..rot {
        (i, j) = (n - 1 - j, i);
    }
    (i, j)
}

/// The n×n output block of one source pixel. Only pixels a blend touches
/// get an accumulator (bit i·6 + j of `touched`); the rest keep the source
/// pixel.
struct Block {
    acc: [[Acc; 6]; 6],
    touched: u64,
    /// The source pixel, as an accumulator.
    solid: Acc,
}

impl Block {
    fn reset(&mut self, solid: Acc) {
        self.touched = 0;
        self.solid = solid;
    }

    fn at(&mut self, i: usize, j: usize) -> &mut Acc {
        let bit = 1u64 << (i * 6 + j);
        if self.touched & bit == 0 {
            self.touched |= bit;
            self.acc[i][j] = self.solid;
        }
        &mut self.acc[i][j]
    }
}

/// `blendPixel<Scaler, ColorDistance, rotDeg>` for the bottom-right corner
/// of the (rotated) kernel: decisions on the canonical kernel `kc`, colours
/// from the original kernel `ko`.
fn blend_pixel(
    kc: &[u8; 9],
    ko: &[u8; 9],
    blend_info: u8,
    rot: usize,
    n: usize,
    m: &Metric,
    out: &mut Block,
) {
    let r = &ROT_KERNEL[rot];
    let [_a, b, c, d, e, f, g, h, i] = r.map(|k| kc[k]);
    let blend = rotate_blend(blend_info, rot);
    let bottom_r = (blend >> 4) & 3;
    if bottom_r < NORMAL {
        return;
    }
    let top_r = (blend >> 2) & 3;
    let bottom_l = (blend >> 6) & 3;
    let do_line = if bottom_r >= DOMINANT {
        true
    } else if (top_r != NONE && !m.eq(e, g)) || (bottom_l != NONE && !m.eq(e, c)) {
        // No second blend in an adjacent rotation (insular pixels), except
        // for 90° corners.
        false
    } else {
        // No full line blend for L-shapes: blend the corner only.
        !(!m.eq(e, i) && m.eq(g, h) && m.eq(h, i) && m.eq(i, f) && m.eq(f, c))
    };
    // Most similar colour, F or H, as the original index.
    let px = if m.d(e, f) <= m.d(e, h) { ko[r[5]] } else { ko[r[7]] };
    let shape = if do_line {
        let fg = m.d(f, g);
        let hc = m.d(h, c);
        let shallow = STEEP_DIRECTION_THRESHOLD * fg <= hc && e != g && d != g;
        let steep = STEEP_DIRECTION_THRESHOLD * hc <= fg && e != c && b != c;
        match (shallow, steep) {
            (true, true) => 2,
            (true, false) => 0,
            (false, true) => 1,
            (false, false) => 3,
        }
    } else {
        4
    };
    for &op in OPS[n - 2][shape] {
        match op {
            G(oi, oj, num, den) => {
                let (bi, bj) = rotate_ij(usize::from(oi), usize::from(oj), rot, n);
                out.at(bi, bj).grad(num, den, px, m);
            }
            S(oi, oj) => {
                let (bi, bj) = rotate_ij(usize::from(oi), usize::from(oj), rot, n);
                *out.at(bi, bj) = Acc::solid(px, m);
            }
        }
    }
}

/// Scales `img` by `n` (2..=6), row-major output `w·n × h·n`. `threads > 1`
/// splits the work into row stripes.
pub fn scale(img: &Image, n: usize, m: &Metric, threads: usize) -> Vec<Mix> {
    let n = n.clamp(2, 6);
    let (w, h) = (img.w, img.h);
    let ow = w * n;
    let mut out = vec![Mix::default(); ow * h * n];
    if w == 0 || h == 0 {
        return out;
    }
    let threads = threads.clamp(1, h);
    let rows_per = h.div_ceil(threads);

    // The image padded by one pixel left and above and two right and below
    // (the edge rule applied once), as original and canonical indices.
    let pw = w + 3;
    let mut orig = vec![0u8; pw * (h + 3)];
    for (k, v) in orig.iter_mut().enumerate() {
        *v = img.at((k % pw) as isize - 1, (k / pw) as isize - 1);
    }
    let canon: Vec<u8> = orig.iter().map(|&v| m.canon[usize::from(v)]).collect();
    let (orig, canon) = (&orig, &canon);
    let at = move |x: usize, y: usize| (y + 1) * pw + x + 1;

    // Pass 1: corner analysis for every pixel.
    let mut res = vec![0u8; w * h];
    std::thread::scope(|s| {
        for (t, chunk) in res.chunks_mut(rows_per * w).enumerate() {
            s.spawn(move || {
                for (k, r) in chunk.iter_mut().enumerate() {
                    let (x, y) = (k % w, t * rows_per + k / w);
                    *r = pre_process(canon, pw, m, at(x, y));
                }
            });
        }
    });

    // Pass 2: fill and blend each pixel's n×n block.
    let res = &res;
    // Blend type of one corner of pixel (x, y)'s pre-pass result; only
    // called with x ≥ −1 and y ≥ −1.
    let corner = move |x: isize, y: isize, shift: u8| -> u8 {
        let x = match x {
            -1 if img.wrap_x => w - 1,
            -1 => return NONE,
            _ => x as usize,
        };
        let y = match y {
            -1 if img.wrap_y => h - 1,
            -1 => return NONE,
            _ => y as usize,
        };
        (res[y * w + x] >> shift) & 3
    };
    std::thread::scope(|s| {
        for (t, chunk) in out.chunks_mut(rows_per * n * ow).enumerate() {
            s.spawn(move || {
                let rows = chunk.len() / (n * ow);
                let mut block = Block { acc: [[Acc::EMPTY; 6]; 6], touched: 0, solid: Acc::EMPTY };
                for ry in 0..rows {
                    let yu = t * rows_per + ry;
                    let y = yu as isize;
                    for xu in 0..w {
                        let x = xu as isize;
                        // TopL, TopR, BottomR, BottomL as xBRZ packs them.
                        let info = corner(x - 1, y - 1, 6)
                            | corner(x, y - 1, 4) << 2
                            | corner(x, y, 0) << 4
                            | corner(x - 1, y, 2) << 6;
                        let c = at(xu, yu);
                        let e = orig[c];
                        let base = ry * n * ow + xu * n;
                        if info == 0 {
                            let px = Mix::plain(e);
                            for i in 0..n {
                                chunk[base + i * ow..base + i * ow + n].fill(px);
                            }
                            continue;
                        }
                        let k = [
                            c - pw - 1,
                            c - pw,
                            c - pw + 1,
                            c - 1,
                            c,
                            c + 1,
                            c + pw - 1,
                            c + pw,
                            c + pw + 1,
                        ];
                        let (kc, ko) = (k.map(|i| canon[i]), k.map(|i| orig[i]));
                        block.reset(Acc::solid(e, m));
                        for rot in 0..4 {
                            blend_pixel(&kc, &ko, info, rot, n, m, &mut block);
                        }
                        let plain = Mix::plain(e);
                        for i in 0..n {
                            for j in 0..n {
                                chunk[base + i * ow + j] =
                                    if block.touched & (1 << (i * 6 + j)) != 0 {
                                        block.acc[i][j].finish(m)
                                    } else {
                                        plain
                                    };
                            }
                        }
                    }
                }
            });
        }
    });
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A grey ramp palette: index i = (i, i, i); 255 is transparent in tests.
    fn grey() -> Palette {
        let mut p = [[0u8; 3]; 256];
        for (i, c) in p.iter_mut().enumerate() {
            *c = [i as u8; 3];
        }
        p
    }

    fn img(px: &[u8], w: usize, h: usize) -> Image<'_> {
        Image { px, w, h, wrap_x: false, wrap_y: false }
    }

    #[test]
    fn flat_image_stays_flat() {
        let m = Metric::new(&grey(), None);
        let px = vec![40u8; 16 * 16];
        for n in 2..=6 {
            let out = scale(&img(&px, 16, 16), n, &m, 1);
            assert_eq!(out.len(), 16 * 16 * n * n);
            assert!(out.iter().all(|&p| p == Mix::plain(40)), "factor {n}");
        }
    }

    #[test]
    fn diagonal_edge_is_smoothed() {
        // Black below the diagonal, white above: xBRZ blends along the edge.
        let m = Metric::new(&grey(), None);
        let mut px = vec![0u8; 16 * 16];
        for y in 0..16 {
            for x in 0..16 {
                if x > y {
                    px[y * 16 + x] = 250;
                }
            }
        }
        let out = scale(&img(&px, 16, 16), 4, &m, 1);
        let blended = out.iter().filter(|p| p.w != 0).count();
        assert!(blended > 0, "edge pixels are blended");
        // Far from the edge nothing changes.
        assert_eq!(out[0], Mix::plain(0));
        assert_eq!(out[63], Mix::plain(250));
        // Only the two source colours occur.
        assert!(out.iter().all(|p| [0, 250].contains(&p.a) && [0, 250].contains(&p.b)));
    }

    #[test]
    fn threads_do_not_change_the_result() {
        let m = Metric::new(&grey(), Some(255));
        let px: Vec<u8> =
            (0..40 * 30).map(|i| [10, 200, 255, 90][(i * 7 / 5 + i / 40) % 4]).collect();
        let one = scale(&img(&px, 40, 30), 3, &m, 1);
        let four = scale(&img(&px, 40, 30), 3, &m, 4);
        assert_eq!(one, four);
    }

    #[test]
    fn transparency_is_resolved_and_never_mixed() {
        // An opaque disc on a transparent background.
        let m = Metric::new(&grey(), Some(255));
        let mut px = vec![255u8; 16 * 16];
        for y in 0..16i32 {
            for x in 0..16i32 {
                if (x - 8) * (x - 8) + (y - 8) * (y - 8) < 25 {
                    px[(y * 16 + x) as usize] = 100;
                }
            }
        }
        let out = scale(&img(&px, 16, 16), 6, &m, 1);
        for p in &out {
            if p.a == 255 {
                assert_eq!(*p, Mix::plain(255), "transparent pixels are plain");
            } else {
                assert_ne!(p.b, 255, "opaque pixels never mix in the transparent index");
            }
        }
        // The disc is rounder: some transparent source pixels gained colour or
        // some opaque ones lost corners.
        let nearest = (0..out.len())
            .filter(|&k| {
                let (x, y) = (k % 96 / 6, k / 96 / 6);
                (px[y * 16 + x] == 255) != (out[k].a == 255)
            })
            .count();
        assert!(nearest > 0);
    }

    #[test]
    fn wrapped_edges_tile_seamlessly() {
        // A vertical stripe crossing the right edge: with wrap the left edge of
        // the output matches the right edge's continuation.
        let m = Metric::new(&grey(), None);
        let mut px = vec![0u8; 8 * 8];
        for y in 0..8 {
            px[y * 8 + ((y + 7) % 8)] = 200;
        }
        let wrapped = scale(&Image { px: &px, w: 8, h: 8, wrap_x: true, wrap_y: true }, 2, &m, 1);
        // Shifting the source by one column and scaling gives the same image
        // shifted by n columns.
        let mut shifted = vec![0u8; 64];
        for y in 0..8 {
            for x in 0..8 {
                shifted[y * 8 + (x + 1) % 8] = px[y * 8 + x];
            }
        }
        let s2 = scale(&Image { px: &shifted, w: 8, h: 8, wrap_x: true, wrap_y: true }, 2, &m, 1);
        for y in 0..16 {
            for x in 0..16 {
                assert_eq!(wrapped[y * 16 + x], s2[y * 16 + (x + 2) % 16], "({x}, {y})");
            }
        }
    }

    /// Timing of a full 320×200 screen (run with `--ignored --nocapture`).
    #[test]
    #[ignore]
    fn bench_screen() {
        let mut pal = [[0u8; 3]; 256];
        for (i, c) in pal.iter_mut().enumerate() {
            *c = [(i * 37 % 256) as u8, (i * 91 % 256) as u8, (i * 13 % 256) as u8];
        }
        let m = Metric::new(&pal, Some(255));
        // Picture-like: blobs of a few colours with dithered edges.
        let px: Vec<u8> = (0..320 * 200)
            .map(|i| {
                let (x, y) = (i % 320, i / 320);
                let v = (x / 7 + y / 5 + (x * y / 97) % 3) % 12;
                (v * 9 + (x ^ y) % 2) as u8
            })
            .collect();
        let img = Image { px: &px, w: 320, h: 200, wrap_x: false, wrap_y: false };
        let pw = 323;
        let mut padded = vec![0u8; pw * 203];
        for (k, v) in padded.iter_mut().enumerate() {
            *v = img.at((k % pw) as isize - 1, (k / pw) as isize - 1);
        }
        let t = std::time::Instant::now();
        let mut blends = 0usize;
        for _ in 0..10 {
            for y in 0..200 {
                for x in 0..320 {
                    blends += usize::from(pre_process(&padded, pw, &m, (y + 1) * pw + x + 1) != 0);
                }
            }
        }
        eprintln!(
            "pass 1 alone: {:.2} ms ({} blended corners)",
            t.elapsed().as_secs_f64() * 100.0,
            blends / 10
        );
        for n in 2..=6 {
            for threads in [1, 8] {
                let t = std::time::Instant::now();
                for _ in 0..10 {
                    let _ = scale(&img, n, &m, threads);
                }
                let ms = t.elapsed().as_secs_f64() * 100.0;
                eprintln!("{n}x, {threads} threads: {ms:.2} ms");
            }
        }
    }

    #[test]
    fn page_is_column_major() {
        let m = Metric::new(&grey(), Some(255));
        let mut page = vec![0u8; 4096];
        page[5 * 64 + 9] = 120; // column 5, row 9
        let hi = HiTexture::from_page(&page, 2, &m, true, false);
        assert_eq!(hi.size(), 128);
        assert_eq!(hi.at(10, 18).a, 120);
        assert_eq!(hi.at(11, 19).a, 120);
        assert_eq!(hi.at(0, 0), Mix::plain(0));
    }
}
