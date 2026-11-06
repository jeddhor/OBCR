//! `AUDIOHED.BC`/`AUDIOT.BC` (PC speaker, AdLib, IMF music; `WOLFSRC/ID_SD.H`)
//! and `AUDIOMUS.BC` (paged digitized sounds; docs/re/DATA_FORMATS.md §5–6).

use crate::bytes::{slice, u8_at, u16_at, u32_at};
use crate::error::{Result, invalid};

/// Audio chunk archive.
#[derive(Debug, Clone)]
pub struct AudioArchive {
    offsets: Vec<u32>,
    data: Vec<u8>,
}

/// PC speaker sound: one PIT-divisor byte per service tick (divisor = byte × 60; 0 = off).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PcSound {
    pub priority: u16,
    pub data: Vec<u8>,
}

/// AdLib instrument (16 bytes, Wolf3D `Instrument`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Instrument {
    pub m_char: u8,
    pub c_char: u8,
    pub m_scale: u8,
    pub c_scale: u8,
    pub m_attack: u8,
    pub c_attack: u8,
    pub m_sus: u8,
    pub c_sus: u8,
    pub m_wave: u8,
    pub c_wave: u8,
    pub n_conn: u8,
    pub voice: u8,
    pub mode: u8,
}

/// AdLib sound effect: instrument, octave block, one note byte per service tick (0 = key off).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdlibSound {
    pub priority: u16,
    pub instrument: Instrument,
    pub block: u8,
    pub data: Vec<u8>,
}

/// One IMF music command: write `value` to OPL register `reg`, then wait `delay` ticks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImfCommand {
    pub reg: u8,
    pub value: u8,
    pub delay: u16,
}

impl AudioArchive {
    pub fn parse(head: &[u8], data: Vec<u8>) -> Result<Self> {
        let offsets: Vec<u32> =
            head.chunks_exact(4).map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect();
        if offsets.windows(2).any(|w| w[1] < w[0])
            || offsets.last().is_some_and(|&o| o as usize > data.len())
        {
            return Err(invalid("AUDIOHED", "offsets not ascending or past end of AUDIOT"));
        }
        Ok(Self { offsets, data })
    }

    pub fn num_chunks(&self) -> usize {
        self.offsets.len().saturating_sub(1)
    }

    /// Raw chunk bytes.
    pub fn chunk(&self, index: usize) -> Result<&[u8]> {
        if index >= self.num_chunks() {
            return Err(invalid("audio chunk", format!("no chunk {index}")));
        }
        let (a, b) = (self.offsets[index] as usize, self.offsets[index + 1] as usize);
        slice(&self.data, a, b - a, "audio chunk")
    }

    pub fn pc_sound(&self, index: usize) -> Result<PcSound> {
        parse_pc_sound(self.chunk(index)?)
    }

    pub fn adlib_sound(&self, index: usize) -> Result<AdlibSound> {
        parse_adlib_sound(self.chunk(index)?)
    }

    pub fn music(&self, index: usize) -> Result<Vec<ImfCommand>> {
        parse_imf(self.chunk(index)?)
    }
}

pub fn parse_pc_sound(chunk: &[u8]) -> Result<PcSound> {
    const WHAT: &str = "PC speaker sound";
    let len = u32_at(chunk, 0, WHAT)? as usize;
    let priority = u16_at(chunk, 4, WHAT)?;
    Ok(PcSound { priority, data: slice(chunk, 6, len, WHAT)?.to_vec() })
}

pub fn parse_adlib_sound(chunk: &[u8]) -> Result<AdlibSound> {
    const WHAT: &str = "AdLib sound";
    let len = u32_at(chunk, 0, WHAT)? as usize;
    let priority = u16_at(chunk, 4, WHAT)?;
    let b = slice(chunk, 6, 16, WHAT)?;
    let instrument = Instrument {
        m_char: b[0],
        c_char: b[1],
        m_scale: b[2],
        c_scale: b[3],
        m_attack: b[4],
        c_attack: b[5],
        m_sus: b[6],
        c_sus: b[7],
        m_wave: b[8],
        c_wave: b[9],
        n_conn: b[10],
        voice: b[11],
        mode: b[12],
    };
    let block = u8_at(chunk, 22, WHAT)?;
    Ok(AdlibSound { priority, instrument, block, data: slice(chunk, 23, len, WHAT)?.to_vec() })
}

/// IMF music: `u16 length_bytes`, then 4-byte commands.
pub fn parse_imf(chunk: &[u8]) -> Result<Vec<ImfCommand>> {
    const WHAT: &str = "IMF music";
    let len = (u16_at(chunk, 0, WHAT)? as usize).min(chunk.len() - 2);
    let body = slice(chunk, 2, len, WHAT)?;
    Ok(body
        .chunks_exact(4)
        .map(|c| ImfCommand { reg: c[0], value: c[1], delay: u16::from_le_bytes([c[2], c[3]]) })
        .collect())
}

/// Page size of `AUDIOMUS`.
pub const AUDIOMUS_PAGE: usize = 4096;

/// `AUDIOMUS.BC`: 4 KiB pages; the last page is a directory of page offsets
/// (u32) and lengths (u16); the last directory entry is the sound map of
/// `(u32 start_page, u32 length)` pairs. Same layout as Corridor 7's
/// `AUDIOMUS.CO7` (EC7Wolf `file_audiomus.cpp`). Samples are 8-bit unsigned mono.
pub fn parse_audiomus(data: &[u8]) -> Result<Vec<Vec<u8>>> {
    const WHAT: &str = "AUDIOMUS";
    if data.len() < 3 * AUDIOMUS_PAGE || !data.len().is_multiple_of(AUDIOMUS_PAGE) {
        return Err(invalid(WHAT, format!("size {} is not a page multiple", data.len())));
    }
    let pages = data.len() / AUDIOMUS_PAGE - 1;
    if 6 * pages > AUDIOMUS_PAGE {
        return Err(invalid(WHAT, "directory does not fit"));
    }
    let dir = &data[pages * AUDIOMUS_PAGE..];
    let mut offsets = Vec::with_capacity(pages);
    let mut lengths = Vec::with_capacity(pages);
    for i in 0..pages {
        offsets.push(u32_at(dir, 4 * i, WHAT)? as usize);
        lengths.push(u16_at(dir, 4 * pages + 2 * i, WHAT)? as usize);
    }
    let map = slice(data, offsets[pages - 1], lengths[pages - 1], WHAT)?;
    (0..map.len() / 8)
        .map(|i| {
            let start = u32_at(map, 8 * i, WHAT)? as usize;
            let length = u32_at(map, 8 * i + 4, WHAT)? as usize;
            let mut samples = Vec::with_capacity(length.min(data.len()));
            let mut page = start;
            while samples.len() < length {
                if page >= pages - 1 {
                    return Err(invalid(WHAT, format!("sound {i} overruns the data pages")));
                }
                let take = lengths[page].min(length - samples.len());
                samples.extend_from_slice(slice(data, offsets[page], take, WHAT)?);
                page += 1;
            }
            Ok(samples)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    proptest::proptest! {
        #[test]
        fn parsers_never_panic(data in proptest::collection::vec(proptest::num::u8::ANY, 0..200)) {
            let _ = parse_pc_sound(&data);
            let _ = parse_adlib_sound(&data);
            let _ = parse_imf(&data);
            let _ = parse_audiomus(&data);
            if let Ok(a) = AudioArchive::parse(&data, data.clone()) {
                for i in 0..a.num_chunks() { let _ = a.chunk(i); }
            }
        }
    }
}
