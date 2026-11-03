//! `VGAHEAD.BC`, `VGADICT.BC`, `VGAGRAPH.BC`: Huffman-compressed pictures and
//! fonts (`WOLFSRC/ID_CA.C` `CAL_HuffExpand`, `ID_VH.H`; docs/re/DATA_FORMATS.md §4).

use crate::bytes::{slice, u8_at, u16_at, u32_at};
use crate::error::{Result, invalid};

/// Root node of the Huffman tree.
const HUFF_ROOT: usize = 254;
/// Largest chunk we will expand (a 320×200 picture is 64000 bytes).
const MAX_CHUNK: usize = 1 << 20;

/// Graphics archive: offsets, Huffman dictionary and compressed data.
#[derive(Debug, Clone)]
pub struct GraphArchive {
    nodes: Vec<(u16, u16)>,
    offsets: Vec<Option<usize>>,
    data: Vec<u8>,
}

/// A picture in linear (row-major) palette indices.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pic {
    pub width: u16,
    pub height: u16,
    pub pixels: Vec<u8>,
}

/// A proportional font (`fontstruct`): glyph rows of 1 byte per pixel, nonzero = ink.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Font {
    pub height: u16,
    pub widths: [u8; 256],
    glyphs: Vec<Vec<u8>>,
}

impl Font {
    pub fn parse(chunk: &[u8]) -> Result<Self> {
        const WHAT: &str = "font";
        let height = u16_at(chunk, 0, WHAT)?;
        let mut widths = [0u8; 256];
        let mut glyphs = Vec::with_capacity(256);
        for (c, w) in widths.iter_mut().enumerate() {
            *w = u8_at(chunk, 2 + 512 + c, WHAT)?;
        }
        for (c, &w) in widths.iter().enumerate() {
            let loc = u16_at(chunk, 2 + 2 * c, WHAT)? as usize;
            let len = w as usize * height as usize;
            glyphs.push(if len == 0 { Vec::new() } else { slice(chunk, loc, len, WHAT)?.to_vec() });
        }
        Ok(Self { height, widths, glyphs })
    }

    /// Row-major glyph pixels (`width × height`).
    pub fn glyph(&self, c: u8) -> &[u8] {
        &self.glyphs[c as usize]
    }

    pub fn width(&self, c: u8) -> u16 {
        self.widths[c as usize] as u16
    }

    /// Width of `text` in pixels.
    pub fn measure(&self, text: &[u8]) -> u32 {
        text.iter().map(|&c| self.widths[c as usize] as u32).sum()
    }
}

impl GraphArchive {
    pub fn parse(head: &[u8], dict: &[u8], data: Vec<u8>) -> Result<Self> {
        let nodes = (0..255)
            .map(|i| Ok((u16_at(dict, 4 * i, "VGADICT")?, u16_at(dict, 4 * i + 2, "VGADICT")?)))
            .collect::<Result<Vec<_>>>()?;
        let offsets = head
            .chunks_exact(3)
            .map(|c| {
                let v = c[0] as usize | (c[1] as usize) << 8 | (c[2] as usize) << 16;
                (v != 0xFF_FFFF).then_some(v)
            })
            .collect();
        Ok(Self { nodes, offsets, data })
    }

    /// Number of chunks (one less than the number of offsets).
    pub fn num_chunks(&self) -> usize {
        self.offsets.len().saturating_sub(1)
    }

    /// Expands chunk `index`; `None` if the chunk is sparse.
    pub fn chunk(&self, index: usize) -> Result<Option<Vec<u8>>> {
        const WHAT: &str = "VGAGRAPH chunk";
        if index >= self.num_chunks() {
            return Err(invalid(WHAT, format!("no chunk {index}")));
        }
        let Some(start) = self.offsets[index] else { return Ok(None) };
        let end =
            self.offsets[index + 1..].iter().flatten().next().copied().unwrap_or(self.data.len());
        if end < start + 4 {
            return Err(invalid(WHAT, format!("chunk {index} too short")));
        }
        let expanded = u32_at(&self.data, start, WHAT)? as usize;
        if expanded > MAX_CHUNK {
            return Err(invalid(WHAT, format!("chunk {index} claims {expanded} bytes")));
        }
        let src = slice(&self.data, start + 4, end - start - 4, WHAT)?;
        self.huff_expand(src, expanded).map(Some)
    }

    fn huff_expand(&self, src: &[u8], len: usize) -> Result<Vec<u8>> {
        let mut out = Vec::with_capacity(len);
        let mut node = HUFF_ROOT;
        'bytes: for &byte in src {
            for bit in 0..8 {
                let (zero, one) = self.nodes[node];
                let next = if byte & (1 << bit) != 0 { one } else { zero };
                if next < 256 {
                    out.push(next as u8);
                    node = HUFF_ROOT;
                    if out.len() == len {
                        break 'bytes;
                    }
                } else {
                    node = (next - 256) as usize;
                    if node >= self.nodes.len() {
                        return Err(invalid("huffman", format!("bad node {next}")));
                    }
                }
            }
        }
        if out.len() != len {
            return Err(invalid("huffman", format!("expanded {} of {len} bytes", out.len())));
        }
        Ok(out)
    }

    /// Reads the picture table from chunk 0: `(width, height)` per picture.
    pub fn pic_table(&self) -> Result<Vec<(u16, u16)>> {
        let table = self.chunk(0)?.ok_or_else(|| invalid("pic table", "chunk 0 is sparse"))?;
        (0..table.len() / 4)
            .map(|i| {
                Ok((u16_at(&table, 4 * i, "pic table")?, u16_at(&table, 4 * i + 2, "pic table")?))
            })
            .collect()
    }
}

/// Converts unchained-VGA planar picture data (four planes, each `width/4 × height`)
/// to linear pixels.
pub fn deplanarize(data: &[u8], width: u16, height: u16) -> Result<Pic> {
    let (w, h) = (width as usize, height as usize);
    if w % 4 != 0 || data.len() < w * h {
        return Err(invalid("pic", format!("{w}x{h} pic with {} bytes", data.len())));
    }
    let quarter = w / 4;
    let mut pixels = vec![0u8; w * h];
    for (i, &v) in data[..w * h].iter().enumerate() {
        let plane = i / (quarter * h);
        let rest = i % (quarter * h);
        let (y, xq) = (rest / quarter, rest % quarter);
        pixels[y * w + xq * 4 + plane] = v;
    }
    Ok(Pic { width, height, pixels })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deplanarize_interleaves_planes() {
        // 4x1 picture: one byte per plane.
        let pic = deplanarize(&[10, 11, 12, 13], 4, 1).unwrap();
        assert_eq!(pic.pixels, vec![10, 11, 12, 13]);
        // 8x1: plane 0 holds x=0,4; plane 1 holds x=1,5; ...
        let pic = deplanarize(&[0, 4, 1, 5, 2, 6, 3, 7], 8, 1).unwrap();
        assert_eq!(pic.pixels, (0..8).collect::<Vec<u8>>());
    }

    proptest::proptest! {
        #[test]
        fn archive_never_panics(head in proptest::collection::vec(proptest::num::u8::ANY, 0..30),
                                dict in proptest::collection::vec(proptest::num::u8::ANY, 1020..1030),
                                data in proptest::collection::vec(proptest::num::u8::ANY, 0..300)) {
            if let Ok(a) = GraphArchive::parse(&head, &dict, data) {
                for i in 0..a.num_chunks() { let _ = a.chunk(i); }
                if let Ok(Some(c)) = a.chunk(0) { let _ = Font::parse(&c); }
            }
        }
    }
}
