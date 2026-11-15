//! `MAPHEAD.BC` and `MAPTEMP.BC` (Wolf3D `CA_CacheMap`, `WOLFSRC/ID_CA.C`;
//! docs/re/DATA_FORMATS.md §3). OBC stores RLEW-only planes; Carmack
//! decompression is provided for other TED5 files and cross-checks.

use crate::bytes::{fixed_str, i32_at, slice, u16_at};
use crate::error::{Result, invalid};

/// Map slots in `MAPHEAD`.
pub const NUM_MAP_SLOTS: usize = 100;
/// Planes per map.
pub const NUM_PLANES: usize = 3;
/// Bytes in a map header (`maptype`).
pub const MAP_HEADER_BYTES: usize = 38;

/// Plane compression used by a map file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Compression {
    /// `MAPTEMP`: expanded length word, then RLEW.
    Rlew,
    /// `GAMEMAPS`: Carmack, then RLEW.
    CarmackRlew,
}

/// One decoded map.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Map {
    pub name: String,
    pub width: u16,
    pub height: u16,
    /// Row-major plane words (`y * width + x`).
    pub planes: [Vec<u16>; NUM_PLANES],
}

impl Map {
    pub fn tile(&self, plane: usize, x: usize, y: usize) -> u16 {
        self.planes[plane][y * self.width as usize + x]
    }
}

/// The map directory plus the map file.
#[derive(Debug, Clone)]
pub struct MapArchive {
    rlew_tag: u16,
    offsets: [i32; NUM_MAP_SLOTS],
    data: Vec<u8>,
    compression: Compression,
}

impl MapArchive {
    pub fn parse(maphead: &[u8], data: Vec<u8>, compression: Compression) -> Result<Self> {
        const WHAT: &str = "MAPHEAD";
        let rlew_tag = u16_at(maphead, 0, WHAT)?;
        let mut offsets = [0i32; NUM_MAP_SLOTS];
        for (i, o) in offsets.iter_mut().enumerate() {
            *o = i32_at(maphead, 2 + 4 * i, WHAT)?;
        }
        Ok(Self { rlew_tag, offsets, data, compression })
    }

    pub fn rlew_tag(&self) -> u16 {
        self.rlew_tag
    }

    /// Slots that hold a map.
    pub fn used_slots(&self) -> impl Iterator<Item = usize> + '_ {
        (0..NUM_MAP_SLOTS).filter(|&i| self.offsets[i] > 0)
    }

    /// Name of the map in `slot` without decoding its planes.
    pub fn name(&self, slot: usize) -> Result<Option<String>> {
        let Some(off) = self.header_offset(slot) else { return Ok(None) };
        Ok(Some(fixed_str(slice(&self.data, off + 22, 16, "map header")?)))
    }

    /// Decodes the map in `slot`, or `None` for an empty slot.
    pub fn map(&self, slot: usize) -> Result<Option<Map>> {
        const WHAT: &str = "map header";
        let Some(off) = self.header_offset(slot) else { return Ok(None) };
        let mut starts = [0i32; NUM_PLANES];
        let mut lengths = [0u16; NUM_PLANES];
        for p in 0..NUM_PLANES {
            starts[p] = i32_at(&self.data, off + 4 * p, WHAT)?;
            lengths[p] = u16_at(&self.data, off + 12 + 2 * p, WHAT)?;
        }
        let width = u16_at(&self.data, off + 18, WHAT)?;
        let height = u16_at(&self.data, off + 20, WHAT)?;
        if width == 0 || height == 0 || width > 256 || height > 256 {
            return Err(invalid(WHAT, format!("map {slot} has size {width}x{height}")));
        }
        let name = fixed_str(slice(&self.data, off + 22, 16, WHAT)?);
        let words = width as usize * height as usize;
        let mut planes: [Vec<u16>; NUM_PLANES] = Default::default();
        for p in 0..NUM_PLANES {
            let start = usize::try_from(starts[p])
                .map_err(|_| invalid(WHAT, format!("map {slot} plane {p} has negative start")))?;
            let src = slice(&self.data, start, lengths[p] as usize, "map plane")?;
            planes[p] = self.decode_plane(src, words)?;
        }
        Ok(Some(Map { name, width, height, planes }))
    }

    fn header_offset(&self, slot: usize) -> Option<usize> {
        let off = *self.offsets.get(slot)?;
        usize::try_from(off).ok().filter(|&o| o > 0)
    }

    fn decode_plane(&self, src: &[u8], words: usize) -> Result<Vec<u16>> {
        let rlew_bytes = match self.compression {
            Compression::Rlew => src.to_vec(),
            Compression::CarmackRlew => {
                let expanded = u16_at(src, 0, "carmack plane")? as usize;
                carmack_expand(&src[2..], expanded / 2)?
                    .iter()
                    .flat_map(|w| w.to_le_bytes())
                    .collect()
            }
        };
        let expanded = u16_at(&rlew_bytes, 0, "rlew plane")? as usize;
        if expanded != words * 2 {
            return Err(invalid(
                "rlew plane",
                format!("expands to {expanded} bytes, expected {}", words * 2),
            ));
        }
        let body: Vec<u16> =
            rlew_bytes[2..].chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect();
        rlew_expand(&body, self.rlew_tag, words)
    }
}

/// Expands RLEW words: `tag, count, value` repeats `value` `count` times.
pub fn rlew_expand(src: &[u16], tag: u16, out_words: usize) -> Result<Vec<u16>> {
    const WHAT: &str = "rlew";
    let mut out = Vec::with_capacity(out_words);
    let mut i = 0;
    while out.len() < out_words {
        let w = *src.get(i).ok_or_else(|| invalid(WHAT, "ran out of input"))?;
        i += 1;
        if w == tag {
            let (Some(&count), Some(&value)) = (src.get(i), src.get(i + 1)) else {
                return Err(invalid(WHAT, "truncated run"));
            };
            i += 2;
            if out.len() + count as usize > out_words {
                return Err(invalid(WHAT, "run overflows plane"));
            }
            out.extend(std::iter::repeat_n(value, count as usize));
        } else {
            out.push(w);
        }
    }
    Ok(out)
}

/// Expands Carmack-compressed bytes to `out_words` words (`WOLFSRC/ID_CA.C` `CAL_CarmackExpand`).
pub fn carmack_expand(src: &[u8], out_words: usize) -> Result<Vec<u16>> {
    const WHAT: &str = "carmack";
    const NEAR: u8 = 0xA7;
    const FAR: u8 = 0xA8;
    let mut out: Vec<u16> = Vec::with_capacity(out_words);
    let mut i = 0;
    while out.len() < out_words {
        let w = u16_at(src, i, WHAT)?;
        i += 2;
        let [count, kind] = w.to_le_bytes();
        if kind == NEAR || kind == FAR {
            if count == 0 {
                // Escaped literal whose high byte is the tag.
                out.push(u16::from_le_bytes([crate::bytes::u8_at(src, i, WHAT)?, kind]));
                i += 1;
                continue;
            }
            let from = if kind == NEAR {
                let back = crate::bytes::u8_at(src, i, WHAT)? as usize;
                i += 1;
                out.len()
                    .checked_sub(back)
                    .ok_or_else(|| invalid(WHAT, "near copy before start"))?
            } else {
                let at = u16_at(src, i, WHAT)? as usize;
                i += 2;
                at
            };
            for k in 0..count as usize {
                let v = *out.get(from + k).ok_or_else(|| invalid(WHAT, "copy past output"))?;
                out.push(v);
            }
        } else {
            out.push(w);
        }
    }
    out.truncate(out_words);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rlew_runs_expand() {
        let tag = 0xABCD;
        assert_eq!(rlew_expand(&[1, tag, 3, 7, 2], tag, 5).unwrap(), vec![1, 7, 7, 7, 2]);
        assert!(rlew_expand(&[tag, 9, 7], tag, 5).is_err());
        assert!(rlew_expand(&[1], tag, 5).is_err());
    }

    #[test]
    fn carmack_copies_and_escapes() {
        // 5, 6, near copy of 2 from 2 back, escaped 0xA7xx literal.
        let src = [5, 0, 6, 0, 2, NEAR_TAG, 2, 0, NEAR_TAG, 0x11];
        assert_eq!(carmack_expand(&src, 5).unwrap(), vec![5, 6, 5, 6, 0xA711]);
    }
    const NEAR_TAG: u8 = 0xA7;

    proptest::proptest! {
        #[test]
        fn decoders_never_panic(data in proptest::collection::vec(proptest::num::u8::ANY, 0..400)) {
            let _ = carmack_expand(&data, 64);
            let words: Vec<u16> = data.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect();
            let _ = rlew_expand(&words, 0xABCD, 64);
            if let Ok(a) = MapArchive::parse(&data, data.clone(), Compression::Rlew) {
                for s in 0..NUM_MAP_SLOTS { let _ = a.map(s); }
            }
        }
    }
}

/// Compresses `words` with RLEW: runs of 4 or more, and any word equal to the
/// tag, become `tag, count, value` (runs of 3 cost the same either way; the
/// original's encoder wrote them as literals).
pub fn rlew_compress(words: &[u16], tag: u16) -> Vec<u16> {
    let mut out = Vec::with_capacity(words.len());
    let mut i = 0;
    while i < words.len() {
        let v = words[i];
        let run = words[i..].iter().take(0xFFFF).take_while(|&&w| w == v).count();
        if run >= 4 || v == tag {
            out.extend_from_slice(&[tag, run as u16, v]);
            i += run;
        } else {
            out.push(v);
            i += 1;
        }
    }
    out
}

/// Writes maps in the original's format: `(MAPHEAD, MAPTEMP)`, RLEW only,
/// a slot per entry of `maps` (`None` leaves it empty). `MAPTEMP` starts with
/// the `TED5v1.0` signature, so no map sits at offset 0 (an empty slot).
pub fn write_archive(maps: &[Option<&Map>], rlew_tag: u16) -> (Vec<u8>, Vec<u8>) {
    let mut head = rlew_tag.to_le_bytes().to_vec();
    let mut body = b"TED5v1.0".to_vec();
    let mut offsets = [0i32; NUM_MAP_SLOTS];
    for (slot, m) in maps.iter().enumerate().take(NUM_MAP_SLOTS) {
        let Some(m) = m else { continue };
        let mut planes = Vec::new();
        for p in &m.planes {
            let mut bytes = ((p.len() * 2) as u16).to_le_bytes().to_vec();
            for w in rlew_compress(p, rlew_tag) {
                bytes.extend_from_slice(&w.to_le_bytes());
            }
            planes.push(bytes);
        }
        let mut starts = [0i32; NUM_PLANES];
        for (p, bytes) in planes.iter().enumerate() {
            starts[p] = body.len() as i32;
            body.extend_from_slice(bytes);
        }
        offsets[slot] = body.len() as i32;
        for s in starts {
            body.extend_from_slice(&s.to_le_bytes());
        }
        for bytes in &planes {
            body.extend_from_slice(&(bytes.len() as u16).to_le_bytes());
        }
        body.extend_from_slice(&m.width.to_le_bytes());
        body.extend_from_slice(&m.height.to_le_bytes());
        let mut name = [0u8; 16];
        for (d, s) in name.iter_mut().zip(m.name.bytes().take(15)) {
            *d = s;
        }
        body.extend_from_slice(&name);
    }
    for o in offsets {
        head.extend_from_slice(&o.to_le_bytes());
    }
    (head, body)
}

#[cfg(test)]
mod writer_tests {
    use super::*;

    #[test]
    fn writes_what_the_reader_reads() {
        let n = 64 * 64;
        let p0: Vec<u16> = (0..n).map(|i| if i % 7 == 0 { 1 } else { 144 }).collect();
        let mut p1 = vec![18u16; n];
        p1[100] = 0xABCD; // the tag itself must survive
        let m =
            Map { name: "TEST MAP".into(), width: 64, height: 64, planes: [p0, p1, vec![0; n]] };
        let (head, body) = write_archive(&[None, Some(&m)], 0xABCD);
        let a = MapArchive::parse(&head, body, Compression::Rlew).unwrap();
        assert_eq!(a.used_slots().collect::<Vec<_>>(), [1]);
        assert_eq!(a.map(1).unwrap().unwrap(), m);
        assert!(a.map(0).unwrap().is_none());
    }
}
