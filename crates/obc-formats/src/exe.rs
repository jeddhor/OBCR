//! `COUNT.EXE`: MZ header and tables read from the user's executable at run
//! time instead of being shipped with OBCR (ADR-005; docs/re/SYMBOLS.md).

use crate::bytes::{slice, u16_at};
use crate::error::{Result, invalid};

/// The executable image (file minus MZ header).
#[derive(Debug, Clone)]
pub struct MzExe {
    data: Vec<u8>,
    header_bytes: usize,
}

impl MzExe {
    pub fn parse(data: Vec<u8>) -> Result<Self> {
        const WHAT: &str = "MZ header";
        if slice(&data, 0, 2, WHAT)? != b"MZ" {
            return Err(invalid(WHAT, "missing MZ signature"));
        }
        let header_bytes = u16_at(&data, 8, WHAT)? as usize * 16;
        if header_bytes > data.len() {
            return Err(invalid(WHAT, "header larger than file"));
        }
        Ok(Self { data, header_bytes })
    }

    pub fn header_bytes(&self) -> usize {
        self.header_bytes
    }

    /// Bytes at image-linear offset `offset` (file offset minus header size).
    pub fn image(&self, offset: usize, len: usize) -> Result<&[u8]> {
        slice(&self.data, self.header_bytes + offset, len, "executable image")
    }

    /// Bytes at a file offset.
    pub fn file(&self, offset: usize, len: usize) -> Result<&[u8]> {
        slice(&self.data, offset, len, "executable")
    }

    /// A 256-color 6-bit VGA palette at `file_offset`, expanded to 8 bits per channel.
    pub fn palette(&self, file_offset: usize) -> Result<[[u8; 3]; 256]> {
        let raw = self.file(file_offset, 768)?;
        if raw.iter().any(|&v| v > 63) {
            return Err(invalid("palette", "value above 63; not a VGA palette"));
        }
        let mut pal = [[0u8; 3]; 256];
        for (i, rgb) in pal.iter_mut().enumerate() {
            for (c, v) in rgb.iter_mut().enumerate() {
                *v = crate::flc::expand6(raw[i * 3 + c]);
            }
        }
        Ok(pal)
    }
}
