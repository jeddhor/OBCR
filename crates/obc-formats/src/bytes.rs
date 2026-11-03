//! Bounds-checked little-endian reads.

use crate::error::{FormatError, Result};

/// Returns `data[offset..offset + len]` or a truncation error naming `what`.
pub(crate) fn slice<'a>(
    data: &'a [u8],
    offset: usize,
    len: usize,
    what: &'static str,
) -> Result<&'a [u8]> {
    offset.checked_add(len).and_then(|end| data.get(offset..end)).ok_or(FormatError::Truncated {
        what,
        offset,
        need: len,
        have: data.len(),
    })
}

pub(crate) fn u8_at(data: &[u8], offset: usize, what: &'static str) -> Result<u8> {
    Ok(slice(data, offset, 1, what)?[0])
}

pub(crate) fn u16_at(data: &[u8], offset: usize, what: &'static str) -> Result<u16> {
    let b = slice(data, offset, 2, what)?;
    Ok(u16::from_le_bytes([b[0], b[1]]))
}

pub(crate) fn i16_at(data: &[u8], offset: usize, what: &'static str) -> Result<i16> {
    u16_at(data, offset, what).map(|v| v as i16)
}

pub(crate) fn u32_at(data: &[u8], offset: usize, what: &'static str) -> Result<u32> {
    let b = slice(data, offset, 4, what)?;
    Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
}

pub(crate) fn i32_at(data: &[u8], offset: usize, what: &'static str) -> Result<i32> {
    u32_at(data, offset, what).map(|v| v as i32)
}

/// Reads a NUL-terminated string from a fixed-size field (bytes after the NUL are ignored).
pub(crate) fn fixed_str(field: &[u8]) -> String {
    let end = field.iter().position(|&b| b == 0).unwrap_or(field.len());
    field[..end].iter().map(|&b| b as char).collect()
}
