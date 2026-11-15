//! The audio-thread mixer.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicI16, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};

use nuked_opl3::Opl3Chip;
use obc_formats::stream::PcmStream;

use crate::synth::{AdlibSfx, ImfPlayer, PcSfx, init_opl};
use crate::{AudioCommand, DIGI_CHANNELS, Pan, Rates, Volumes};

/// Sends commands to a [`Mixer`] from the game thread.
#[derive(Clone)]
pub struct MixerHandle {
    tx: Sender<AudioCommand>,
    /// The CD drive: shared with the mixer, outlives any one screen.
    pub cd: Arc<CdDrive>,
}

impl MixerHandle {
    pub fn send(&self, cmd: AudioCommand) {
        // A closed channel means audio is gone; the game keeps running silently.
        let _ = self.tx.send(cmd);
    }
}

/// What the CD release keeps about its drive (docs/re/CD_RELEASE.md section 4):
/// `busy` is what the drive reports (set by the mixer while a track plays);
/// `track` and `playing` are the game's own variables.
#[derive(Debug)]
pub struct CdDrive {
    busy: AtomicBool,
    /// Current track, 1..=4 (CD tracks 2..5); DS:0x2CF4, initially 1.
    pub track: AtomicI16,
    /// A track was started (`CD_Play`); DS:0x2CF8.
    pub playing: AtomicBool,
}

impl Default for CdDrive {
    fn default() -> Self {
        Self {
            busy: AtomicBool::new(false),
            track: AtomicI16::new(1),
            playing: AtomicBool::new(false),
        }
    }
}

impl CdDrive {
    /// `CD_Busy`: a track is playing.
    pub fn busy(&self) -> bool {
        self.busy.load(Ordering::Relaxed)
    }

    /// Marks a track as started before the mixer has picked it up, so a
    /// check in the same tic does not start it again.
    pub fn mark_busy(&self) {
        self.busy.store(true, Ordering::Relaxed);
    }
}

/// Streamed music: a CD track (QOA, played once) or a mod's music (QOA or
/// Ogg Vorbis, looped), decoded a block at a time and resampled linearly to
/// the output rate.
struct StreamVoice {
    src: PcmStream,
    looped: bool,
    channels: usize,
    /// Interleaved samples; the first frame is the previous block's last.
    buf: Vec<i16>,
    tmp: Vec<i16>,
    pos: f64,
    step: f64,
}

impl StreamVoice {
    fn new(data: Arc<[u8]>, out_rate: u32, looped: bool) -> Option<Self> {
        let src = PcmStream::open(data).ok()?;
        let (channels, rate) = (src.channels(), src.rate());
        Some(Self {
            src,
            looped,
            channels,
            buf: Vec::new(),
            tmp: Vec::new(),
            pos: 0.0,
            step: f64::from(rate) / f64::from(out_rate),
        })
    }

    /// The next block; at the end, the first block again when looping.
    fn refill(&mut self) -> bool {
        if self.src.next_block(&mut self.tmp) {
            return true;
        }
        if !self.looped {
            return false;
        }
        self.src.rewind();
        self.src.next_block(&mut self.tmp)
    }

    /// The next output sample pair, `None` at the end of the track.
    fn next(&mut self) -> Option<(f32, f32)> {
        let ch = self.channels;
        while self.pos as usize + 1 >= self.buf.len() / ch {
            let frames = self.buf.len() / ch;
            if !self.refill() {
                return None;
            }
            let mut next = Vec::with_capacity(self.tmp.len() + ch);
            if frames > 0 {
                next.extend_from_slice(&self.buf[(frames - 1) * ch..]);
                self.pos -= (frames - 1) as f64;
            }
            next.extend_from_slice(&self.tmp);
            self.buf = next;
        }
        let i = self.pos as usize;
        let frac = (self.pos - i as f64) as f32;
        let at = |f: usize, c: usize| f32::from(self.buf[f * ch + c.min(ch - 1)]) / 32768.0;
        let l = at(i, 0) + (at(i + 1, 0) - at(i, 0)) * frac;
        let r = at(i, 1) + (at(i + 1, 1) - at(i, 1)) * frac;
        self.pos += self.step;
        Some((l, r))
    }
}

struct DigiVoice {
    samples: Arc<[u8]>,
    pos: f64,
    step: f64,
    pan: Pan,
}

/// Synthesizes interleaved stereo `f32` at a fixed output rate.
pub struct Mixer {
    rate: u32,
    rx: Receiver<AudioCommand>,
    opl: Opl3Chip,
    rates: Rates,
    volumes: Volumes,
    adlib: Option<AdlibSfx>,
    pc: Option<PcSfx>,
    pc_phase: f64,
    music: Option<ImfPlayer>,
    music_paused: bool,
    sfx_clock: f64,
    music_clock: f64,
    digi: [Option<DigiVoice>; DIGI_CHANNELS],
    opl_buf: [i16; 2],
    cd: Option<StreamVoice>,
    drive: Arc<CdDrive>,
    /// A mod's music file, looping.
    stream: Option<StreamVoice>,
}

impl Mixer {
    pub fn new(rate: u32) -> (Self, MixerHandle) {
        let (tx, rx) = channel();
        let drive = Arc::new(CdDrive::default());
        let mut opl = Opl3Chip::new(rate);
        init_opl(&mut opl);
        let mixer = Self {
            rate,
            rx,
            opl,
            rates: Rates::default(),
            volumes: Volumes::default(),
            adlib: None,
            pc: None,
            pc_phase: 0.0,
            music: None,
            music_paused: false,
            sfx_clock: 0.0,
            music_clock: 0.0,
            digi: Default::default(),
            opl_buf: [0; 2],
            cd: None,
            drive: drive.clone(),
            stream: None,
        };
        (mixer, MixerHandle { tx, cd: drive })
    }

    fn apply(&mut self, cmd: AudioCommand) {
        match cmd {
            AudioCommand::PlayAdlib(s) => self.adlib = Some(AdlibSfx::start(&mut self.opl, s)),
            AudioCommand::StopAdlib => {
                AdlibSfx::stop(&mut self.opl);
                self.adlib = None;
            }
            AudioCommand::PlayPc(s) => self.pc = Some(PcSfx::start(s)),
            AudioCommand::StopPc => self.pc = None,
            AudioCommand::PlayDigi { channel, samples, rate, pan } => {
                if let Some(slot) = self.digi.get_mut(channel) {
                    *slot = Some(DigiVoice {
                        samples,
                        pos: 0.0,
                        step: f64::from(rate) / f64::from(self.rate),
                        pan,
                    });
                }
            }
            AudioCommand::PanDigi { channel, pan } => {
                if let Some(Some(v)) = self.digi.get_mut(channel) {
                    v.pan = pan;
                }
            }
            AudioCommand::StopDigi { channel } => {
                if let Some(slot) = self.digi.get_mut(channel) {
                    *slot = None;
                }
            }
            // A mod's music stream is music too: a new song replaces it and
            // stopping or pausing music stops or pauses it.
            AudioCommand::StartMusic(commands) => {
                ImfPlayer::silence(&mut self.opl);
                self.music_paused = false;
                self.music = Some(ImfPlayer::new(commands));
                self.stream = None;
            }
            AudioCommand::StopMusic => {
                ImfPlayer::silence(&mut self.opl);
                self.music = None;
                self.music_paused = false;
                self.stream = None;
            }
            AudioCommand::PauseMusic(on) => {
                if on {
                    ImfPlayer::silence(&mut self.opl);
                }
                self.music_paused = on;
            }
            AudioCommand::PlayCd(data) => {
                self.cd = StreamVoice::new(data, self.rate, false);
                self.drive.busy.store(self.cd.is_some(), Ordering::Relaxed);
            }
            AudioCommand::StopCd => {
                self.cd = None;
                self.drive.busy.store(false, Ordering::Relaxed);
            }
            AudioCommand::PlayStream(data) => {
                self.music_paused = false;
                self.stream = StreamVoice::new(data, self.rate, true);
            }
            AudioCommand::StopStream => self.stream = None,
            AudioCommand::SetVolumes(v) => self.volumes = v,
            AudioCommand::SetRates(r) => self.rates = r,
        }
    }

    /// True while any digitized channel is playing.
    pub fn digi_playing(&self, channel: usize) -> bool {
        self.digi.get(channel).is_some_and(Option::is_some)
    }

    /// Fills interleaved stereo samples.
    pub fn fill(&mut self, out: &mut [f32]) {
        while let Ok(cmd) = self.rx.try_recv() {
            self.apply(cmd);
        }
        let dt = 1.0 / f64::from(self.rate);
        let v = self.volumes;
        for frame in out.chunks_exact_mut(2) {
            self.sfx_clock += dt * self.rates.sfx_hz;
            while self.sfx_clock >= 1.0 {
                self.sfx_clock -= 1.0;
                if let Some(a) = &mut self.adlib
                    && !a.tick(&mut self.opl)
                {
                    self.adlib = None;
                }
                if let Some(p) = &mut self.pc
                    && !p.tick()
                {
                    self.pc = None;
                }
            }
            self.music_clock += dt * self.rates.imf_hz;
            while self.music_clock >= 1.0 {
                self.music_clock -= 1.0;
                if let Some(m) = &mut self.music
                    && !self.music_paused
                {
                    m.tick(&mut self.opl);
                }
            }

            // OPL output carries both music and AdLib effects; weight by the louder role.
            self.opl.generate_resampled(&mut self.opl_buf).ok();
            let opl_gain = if self.adlib.is_some() { v.sfx.max(v.music) } else { v.music };
            let mut l = f32::from(self.opl_buf[0]) / 32768.0 * opl_gain * 2.0;
            let mut r = f32::from(self.opl_buf[1]) / 32768.0 * opl_gain * 2.0;

            if let Some(p) = &self.pc
                && p.freq > 0.0
            {
                self.pc_phase = (self.pc_phase + p.freq * dt).fract();
                let s = if self.pc_phase < 0.5 { 0.15 } else { -0.15 } * v.sfx;
                l += s;
                r += s;
            }

            if let Some(cd) = &mut self.cd {
                match cd.next() {
                    Some((cl, cr)) => {
                        l += cl * v.music;
                        r += cr * v.music;
                    }
                    None => {
                        self.cd = None;
                        self.drive.busy.store(false, Ordering::Relaxed);
                    }
                }
            }

            if let Some(st) = self.stream.as_mut().filter(|_| !self.music_paused) {
                match st.next() {
                    Some((sl, sr)) => {
                        l += sl * v.music;
                        r += sr * v.music;
                    }
                    None => self.stream = None,
                }
            }

            for slot in &mut self.digi {
                let Some(voice) = slot else { continue };
                let i = voice.pos as usize;
                if i + 1 >= voice.samples.len() {
                    *slot = None;
                    continue;
                }
                let frac = (voice.pos - i as f64) as f32;
                let a = (f32::from(voice.samples[i]) - 128.0) / 128.0;
                let b = (f32::from(voice.samples[i + 1]) - 128.0) / 128.0;
                let s = (a + (b - a) * frac) * v.sfx;
                let lv = f32::from(15 - voice.pan.left.min(15)) / 15.0;
                let rv = f32::from(15 - voice.pan.right.min(15)) / 15.0;
                l += s * lv;
                r += s * rv;
                voice.pos += voice.step;
            }

            frame[0] = (l * v.master).clamp(-1.0, 1.0);
            frame[1] = (r * v.master).clamp(-1.0, 1.0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AdlibSound, PcSound};
    use obc_formats::audio::Instrument;

    #[test]
    fn plays_all_sources_without_panicking() {
        let (mut mixer, handle) = Mixer::new(48_000);
        handle.send(AudioCommand::PlayPc(Arc::new(PcSound { priority: 1, data: vec![30, 0, 40] })));
        handle.send(AudioCommand::PlayAdlib(Arc::new(AdlibSound {
            priority: 1,
            instrument: Instrument { m_char: 1, c_char: 1, ..Default::default() },
            block: 4,
            data: vec![100, 120, 0],
        })));
        handle.send(AudioCommand::PlayDigi {
            channel: 0,
            samples: vec![128, 200, 50, 128].into(),
            rate: 9009,
            pan: Pan::default(),
        });
        let mut buf = vec![0.0; 4800];
        mixer.fill(&mut buf);
        assert!(buf.iter().any(|&s| s != 0.0));
        assert!(buf.iter().all(|s| s.is_finite()));
    }

    #[test]
    fn streams_a_cd_track_to_its_end() {
        let (mut mixer, handle) = Mixer::new(48_000);
        // 0.25 s of a 44.1 kHz ramp, more than one QOA frame; channels differ.
        let pcm: Vec<i16> =
            (0..11_025).flat_map(|i| [(i % 2000) as i16 * 8, -((i % 2000) as i16) * 8]).collect();
        let qoa = obc_formats::qoa::encode(&pcm, 2, 44_100).unwrap();
        handle.send(AudioCommand::PlayCd(qoa.into()));
        let mut buf = vec![0.0; 2 * 4_800];
        mixer.fill(&mut buf);
        assert!(handle.cd.busy());
        assert!(buf.chunks_exact(2).any(|f| f[0] > 0.05 && f[1] < -0.05));
        let mut rest = vec![0.0; 2 * 9_000];
        mixer.fill(&mut rest);
        assert!(!handle.cd.busy(), "0.29 s of output ends a 0.25 s track");
        assert_eq!(rest[rest.len() - 2..], [0.0, 0.0]);
    }

    #[test]
    fn a_music_stream_loops() {
        let (mut mixer, handle) = Mixer::new(48_000);
        // 0.1 s of 22.05 kHz mono: looping keeps it sounding long after.
        let pcm: Vec<i16> =
            (0..2_205).map(|i| if i % 50 < 25 { 12_000 } else { -12_000 }).collect();
        let qoa = obc_formats::qoa::encode(&pcm, 1, 22_050).unwrap();
        handle.send(AudioCommand::PlayStream(qoa.into()));
        let mut buf = vec![0.0; 2 * 48_000];
        mixer.fill(&mut buf);
        let tail = &buf[buf.len() - 2 * 4_800..];
        assert!(tail.iter().any(|&s| s.abs() > 0.1), "still playing after 1 s");
        assert!(!handle.cd.busy(), "a stream is not the CD drive");
        handle.send(AudioCommand::PauseMusic(true));
        mixer.fill(&mut buf);
        assert_eq!(buf[buf.len() - 2..], [0.0, 0.0], "paused");
        handle.send(AudioCommand::PauseMusic(false));
        mixer.fill(&mut buf);
        assert!(buf.iter().any(|&s| s.abs() > 0.1), "resumed");
        handle.send(AudioCommand::StopMusic);
        mixer.fill(&mut buf);
        assert_eq!(buf[buf.len() - 2..], [0.0, 0.0], "stopping music stops the stream");
        // Ogg Vorbis streams too.
        let ogg: &[u8] = include_bytes!("../../obc-formats/tests/data/tone.ogg");
        handle.send(AudioCommand::PlayStream(Arc::from(ogg)));
        let mut buf = vec![0.0; 2 * 4_800];
        mixer.fill(&mut buf);
        assert!(buf.iter().any(|&s| s.abs() > 0.05));
    }
}
