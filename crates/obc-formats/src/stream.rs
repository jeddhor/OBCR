//! Streamed music: QOA (the ripped CD tracks, and mod music) and Ogg Vorbis
//! (mod music), decoded a block at a time for the mixer
//! (docs/MOD_ASSETS_AND_ARENAS.md §3.4).
//!
//! A stream holds the whole compressed file and decodes on demand, so the
//! audio thread never decodes more than one block per refill.

use std::io::Cursor;
use std::sync::Arc;

use lewton::inside_ogg::OggStreamReader;

use crate::error::{Result, invalid};
use crate::qoa;

/// What a music file is, from its first bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamKind {
    Qoa,
    Vorbis,
}

impl StreamKind {
    pub fn sniff(data: &[u8]) -> Option<Self> {
        match data.get(..4)? {
            b"qoaf" => Some(Self::Qoa),
            b"OggS" => Some(Self::Vorbis),
            _ => None,
        }
    }
}

enum Inner {
    Qoa(qoa::Decoder<Arc<[u8]>>),
    Vorbis(Box<OggStreamReader<Cursor<Arc<[u8]>>>>),
}

/// A music file being decoded: interleaved 16-bit blocks.
pub struct PcmStream {
    data: Arc<[u8]>,
    inner: Inner,
    channels: usize,
    rate: u32,
}

impl std::fmt::Debug for PcmStream {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PcmStream")
            .field("bytes", &self.data.len())
            .field("channels", &self.channels)
            .field("rate", &self.rate)
            .finish()
    }
}

fn open_vorbis(data: &Arc<[u8]>) -> Result<OggStreamReader<Cursor<Arc<[u8]>>>> {
    OggStreamReader::new(Cursor::new(Arc::clone(data)))
        .map_err(|e| invalid("Ogg Vorbis", e.to_string()))
}

impl PcmStream {
    /// Opens a QOA or Ogg Vorbis file (by its magic).
    pub fn open(data: Arc<[u8]>) -> Result<Self> {
        match StreamKind::sniff(&data) {
            Some(StreamKind::Qoa) => {
                let dec = qoa::Decoder::new(Arc::clone(&data))?;
                let info = dec.info();
                Ok(Self { data, inner: Inner::Qoa(dec), channels: info.channels, rate: info.rate })
            }
            Some(StreamKind::Vorbis) => {
                let r = open_vorbis(&data)?;
                let channels = usize::from(r.ident_hdr.audio_channels);
                let rate = r.ident_hdr.audio_sample_rate;
                if channels == 0 || rate == 0 {
                    return Err(invalid("Ogg Vorbis", "no channels or no sample rate"));
                }
                Ok(Self { data, inner: Inner::Vorbis(Box::new(r)), channels, rate })
            }
            None => Err(invalid("music", "neither a QOA nor an Ogg Vorbis file")),
        }
    }

    pub fn kind(&self) -> StreamKind {
        match self.inner {
            Inner::Qoa(_) => StreamKind::Qoa,
            Inner::Vorbis(_) => StreamKind::Vorbis,
        }
    }

    pub fn channels(&self) -> usize {
        self.channels
    }

    pub fn rate(&self) -> u32 {
        self.rate
    }

    /// Decodes the next block into `out` (interleaved; cleared first).
    /// `false` at the end of the file or on damage.
    pub fn next_block(&mut self, out: &mut Vec<i16>) -> bool {
        out.clear();
        match &mut self.inner {
            Inner::Qoa(d) => d.next_frame(out),
            Inner::Vorbis(r) => {
                // Header and empty packets decode to nothing: skip them, with
                // a bound so a hostile file cannot spin forever.
                for _ in 0..64 {
                    match r.read_dec_packet_itl() {
                        Ok(Some(p)) if p.is_empty() => continue,
                        Ok(Some(p)) => {
                            // A changed channel count mid-stream would garble
                            // the interleaving: treat it as the end.
                            if p.len() % self.channels != 0 {
                                return false;
                            }
                            *out = p;
                            return true;
                        }
                        Ok(None) | Err(_) => return false,
                    }
                }
                false
            }
        }
    }

    /// Starts again from the beginning.
    pub fn rewind(&mut self) {
        match &mut self.inner {
            Inner::Qoa(d) => d.rewind(),
            Inner::Vorbis(r) => {
                if let Ok(fresh) = open_vorbis(&self.data) {
                    **r = fresh;
                }
            }
        }
    }

    /// Decodes the whole file once (a mod check): its length in samples per
    /// channel. Errors when a block is damaged before the end or the file
    /// holds no audio.
    pub fn verify(mut self, max_seconds: u32) -> Result<u64> {
        let mut block = Vec::new();
        let mut frames = 0u64;
        let limit = u64::from(max_seconds) * u64::from(self.rate);
        while self.next_block(&mut block) {
            frames += (block.len() / self.channels) as u64;
            if frames > limit {
                return Err(invalid("music", format!("longer than {max_seconds} seconds")));
            }
        }
        if frames == 0 {
            return Err(invalid("music", "no audio decoded"));
        }
        if let Inner::Qoa(d) = &self.inner {
            let want = u64::from(d.info().samples);
            if want != 0 && frames < want {
                return Err(invalid("QOA", format!("damaged after {frames} of {want} samples")));
            }
        }
        Ok(frames)
    }
}
