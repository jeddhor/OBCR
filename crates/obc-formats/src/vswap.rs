//! `GFXTILES.BC`: the page file of walls, sprites and digitized sounds
//! (Wolf3D `VSWAP`, `WOLFSRC/ID_PM.C`; docs/re/DATA_FORMATS.md §2).

use crate::bytes::{i16_at, slice, u16_at, u32_at};
use crate::error::{Result, invalid};

/// Side of a wall page or sprite shape, in pixels.
pub const SHAPE_SIZE: usize = 64;
/// Bytes in a wall page.
pub const WALL_BYTES: usize = SHAPE_SIZE * SHAPE_SIZE;

/// Parsed page file. Pages are borrowed from the owned file data.
#[derive(Debug, Clone)]
pub struct PageFile {
    data: Vec<u8>,
    offsets: Vec<u32>,
    lengths: Vec<u16>,
    sprite_start: usize,
    sound_start: usize,
}

/// A 64×64 sprite decoded to column-major indexed pixels plus an opacity mask.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Shape {
    /// First and last non-empty columns (`leftpix`, `rightpix`).
    pub left: u16,
    pub right: u16,
    /// Column-major palette indices (`x * 64 + y`); transparent pixels are 0.
    pub pixels: Vec<u8>,
    /// Column-major opacity, parallel to `pixels`.
    pub opaque: Vec<bool>,
    /// Per column, the drawn runs as `(start_y, end_y_exclusive, source_offset)`
    /// in original post order. Renderers that mirror `ScaleShape` use these.
    pub posts: Vec<Vec<Post>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Post {
    pub start: u16,
    pub end: u16,
}

/// A digitized sound assembled from consecutive sound pages.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DigiSound {
    /// First page, relative to the sound start.
    pub start_page: u16,
    /// 8-bit unsigned mono samples.
    pub samples: Vec<u8>,
}

impl PageFile {
    pub fn parse(data: Vec<u8>) -> Result<Self> {
        const WHAT: &str = "page file";
        let count = u16_at(&data, 0, WHAT)? as usize;
        let sprite_start = u16_at(&data, 2, WHAT)? as usize;
        let sound_start = u16_at(&data, 4, WHAT)? as usize;
        if !(sprite_start <= sound_start && sound_start <= count) {
            return Err(invalid(WHAT, format!("bad ranges {sprite_start}/{sound_start}/{count}")));
        }
        let mut offsets = Vec::with_capacity(count);
        let mut lengths = Vec::with_capacity(count);
        for i in 0..count {
            offsets.push(u32_at(&data, 6 + 4 * i, WHAT)?);
            lengths.push(u16_at(&data, 6 + 4 * count + 2 * i, WHAT)?);
        }
        for (i, (&off, &len)) in offsets.iter().zip(&lengths).enumerate() {
            if off != 0 {
                slice(&data, off as usize, len as usize, "page file page")
                    .map_err(|_| invalid(WHAT, format!("page {i} out of bounds")))?;
            }
        }
        Ok(Self { data, offsets, lengths, sprite_start, sound_start })
    }

    pub fn num_pages(&self) -> usize {
        self.offsets.len()
    }

    pub fn num_walls(&self) -> usize {
        self.sprite_start
    }

    pub fn num_sprites(&self) -> usize {
        self.sound_start - self.sprite_start
    }

    pub fn sprite_start(&self) -> usize {
        self.sprite_start
    }

    pub fn sound_start(&self) -> usize {
        self.sound_start
    }

    /// Raw page bytes (empty for sparse pages).
    pub fn page(&self, index: usize) -> Result<&[u8]> {
        let off = *self
            .offsets
            .get(index)
            .ok_or_else(|| invalid("page file", format!("no page {index}")))?;
        if off == 0 {
            return Ok(&[]);
        }
        slice(&self.data, off as usize, self.lengths[index] as usize, "page file page")
    }

    /// Wall page `index`: 4096 column-major palette indices.
    pub fn wall(&self, index: usize) -> Result<&[u8]> {
        if index >= self.sprite_start {
            return Err(invalid("wall", format!("no wall {index}")));
        }
        let page = self.page(index)?;
        if page.len() != WALL_BYTES {
            return Err(invalid("wall", format!("wall {index} is {} bytes", page.len())));
        }
        Ok(page)
    }

    /// Decodes sprite `index` (0-based from the sprite start).
    pub fn sprite(&self, index: usize) -> Result<Shape> {
        if index >= self.num_sprites() {
            return Err(invalid("sprite", format!("no sprite {index}")));
        }
        decode_shape(self.page(self.sprite_start + index)?)
    }

    /// All digitized sounds listed in the sound info page (the last page).
    pub fn digi_sounds(&self) -> Result<Vec<DigiSound>> {
        const WHAT: &str = "digitized sound list";
        let count = self.num_pages();
        if count == 0 || self.sound_start >= count {
            return Ok(Vec::new());
        }
        let info = self.page(count - 1)?;
        let entries: Vec<(u16, u16)> = (0..info.len() / 4)
            .map(|i| Ok((u16_at(info, 4 * i, WHAT)?, u16_at(info, 4 * i + 2, WHAT)?)))
            .collect::<Result<_>>()?;
        let sound_pages = count - 1 - self.sound_start;
        entries
            .iter()
            .map(|&(start_page, length)| {
                let mut samples = Vec::with_capacity(length as usize);
                let mut page = start_page as usize;
                while samples.len() < length as usize {
                    if page >= sound_pages {
                        return Err(invalid(WHAT, format!("sound at page {start_page} overruns")));
                    }
                    let bytes = self.page(self.sound_start + page)?;
                    let take = bytes.len().min(length as usize - samples.len());
                    samples.extend_from_slice(&bytes[..take]);
                    page += 1;
                }
                Ok(DigiSound { start_page, samples })
            })
            .collect()
    }
}

impl Shape {
    /// A sprite from a 64×64 column-major image (`x * 64 + y`) whose
    /// transparent pixels are `transparent`: the same shape [`decode_shape`]
    /// would give for a page encoding it (a mod's replacement sprite,
    /// docs/MOD_ASSETS_AND_ARENAS.md §3.2). Posts are the opaque runs of each
    /// column, top to bottom.
    pub fn from_indexed(columns: &[u8], transparent: u8) -> Self {
        assert_eq!(columns.len(), WALL_BYTES, "a sprite image is 64x64");
        let opaque: Vec<bool> = columns.iter().map(|&p| p != transparent).collect();
        let pixels: Vec<u8> =
            columns.iter().zip(&opaque).map(|(&p, &o)| if o { p } else { 0 }).collect();
        let mut posts = vec![Vec::new(); SHAPE_SIZE];
        for (x, col) in posts.iter_mut().enumerate() {
            let mut y = 0;
            while y < SHAPE_SIZE {
                if opaque[x * SHAPE_SIZE + y] {
                    let start = y;
                    while y < SHAPE_SIZE && opaque[x * SHAPE_SIZE + y] {
                        y += 1;
                    }
                    col.push(Post { start: start as u16, end: y as u16 });
                } else {
                    y += 1;
                }
            }
        }
        let used: Vec<usize> = (0..SHAPE_SIZE).filter(|&x| !posts[x].is_empty()).collect();
        let (left, right) = match (used.first(), used.last()) {
            (Some(&l), Some(&r)) => (l as u16, r as u16),
            _ => (0, 0),
        };
        Self { left, right, pixels, opaque, posts }
    }
}

/// Decodes a Wolf3D `t_compshape`: `leftpix`, `rightpix`, column offsets, then
/// per-column post lists of `(end*2, corrected_top, start*2)` ending with 0.
pub fn decode_shape(page: &[u8]) -> Result<Shape> {
    const WHAT: &str = "sprite";
    let left = u16_at(page, 0, WHAT)?;
    let right = u16_at(page, 2, WHAT)?;
    if left > right || right as usize >= SHAPE_SIZE {
        return Err(invalid(WHAT, format!("bad column range {left}..={right}")));
    }
    let mut pixels = vec![0u8; WALL_BYTES];
    let mut opaque = vec![false; WALL_BYTES];
    let mut posts = vec![Vec::new(); SHAPE_SIZE];
    for x in left..=right {
        let mut cmd = u16_at(page, 4 + 2 * (x - left) as usize, WHAT)? as usize;
        // A column cannot hold more posts than rows; bound the loop for hostile input.
        for _ in 0..=SHAPE_SIZE {
            let end = u16_at(page, cmd, WHAT)? / 2;
            if end == 0 {
                break;
            }
            let top = i16_at(page, cmd + 2, WHAT)? as i32;
            let start = u16_at(page, cmd + 4, WHAT)? / 2;
            cmd += 6;
            if start > end || end as usize > SHAPE_SIZE {
                return Err(invalid(WHAT, format!("bad post {start}..{end} in column {x}")));
            }
            for y in start..end {
                let src = usize::try_from(top + y as i32)
                    .map_err(|_| invalid(WHAT, "negative pixel offset"))?;
                let i = x as usize * SHAPE_SIZE + y as usize;
                pixels[i] = crate::bytes::u8_at(page, src, WHAT)?;
                opaque[i] = true;
            }
            posts[x as usize].push(Post { start, end });
        }
    }
    Ok(Shape { left, right, pixels, opaque, posts })
}

#[cfg(test)]
mod tests {
    use super::{Shape, decode_shape};

    /// Encodes a shape as a `t_compshape` page (test helper).
    fn encode(sh: &Shape) -> Vec<u8> {
        let (l, r) = (sh.left as usize, sh.right as usize);
        let mut page = vec![0u8; 4 + 2 * (r - l + 1)];
        page[0..2].copy_from_slice(&sh.left.to_le_bytes());
        page[2..4].copy_from_slice(&sh.right.to_le_bytes());
        // Pixel bytes first, then the post lists; `top` points into them.
        let mut cols = Vec::new();
        for x in l..=r {
            let mut list = Vec::new();
            for p in &sh.posts[x] {
                let src = page.len() as i32;
                for y in p.start..p.end {
                    page.push(sh.pixels[x * 64 + y as usize]);
                }
                list.push((p.end * 2, (src - i32::from(p.start)) as i16, p.start * 2));
            }
            cols.push(list);
        }
        for (k, list) in cols.into_iter().enumerate() {
            let at = page.len() as u16;
            page[4 + 2 * k..6 + 2 * k].copy_from_slice(&at.to_le_bytes());
            for (e, t, s) in list {
                page.extend_from_slice(&e.to_le_bytes());
                page.extend_from_slice(&t.to_le_bytes());
                page.extend_from_slice(&s.to_le_bytes());
            }
            page.extend_from_slice(&0u16.to_le_bytes());
        }
        page
    }

    #[test]
    fn from_indexed_matches_decoding_an_encoded_page() {
        let mut img = vec![0xFFu8; 64 * 64];
        // Two runs in column 10, one in column 40, a pixel in column 63.
        for y in 5..9 {
            img[10 * 64 + y] = y as u8;
        }
        for y in 20..64 {
            img[10 * 64 + y] = 200;
        }
        img[40 * 64] = 1;
        img[63 * 64 + 63] = 0; // index 0 is a colour, not transparency
        let sh = Shape::from_indexed(&img, 0xFF);
        assert_eq!((sh.left, sh.right), (10, 63));
        assert_eq!(sh.posts[10].len(), 2);
        assert_eq!(decode_shape(&encode(&sh)).unwrap(), sh);
        let empty = Shape::from_indexed(&vec![0xFF; 64 * 64], 0xFF);
        assert!(empty.posts.iter().all(Vec::is_empty));
    }

    use super::*;

    #[test]
    fn rejects_garbage_without_panicking() {
        assert!(PageFile::parse(vec![]).is_err());
        assert!(PageFile::parse(vec![5, 0, 9, 0, 1, 0]).is_err());
        assert!(decode_shape(&[0, 0, 70, 0]).is_err());
    }

    proptest::proptest! {
        #[test]
        fn shape_decoder_never_panics(data in proptest::collection::vec(proptest::num::u8::ANY, 0..600)) {
            let _ = decode_shape(&data);
        }

        #[test]
        fn page_file_parser_never_panics(data in proptest::collection::vec(proptest::num::u8::ANY, 0..600)) {
            if let Ok(pf) = PageFile::parse(data) {
                for i in 0..pf.num_sprites() { let _ = pf.sprite(i); }
                let _ = pf.digi_sounds();
            }
        }
    }
}
