//! Everything loaded from the original data at startup.

use obc_formats::audio::{AudioArchive, parse_audiomus};
use obc_formats::exe::MzExe;
use obc_formats::maps::{Compression, MapArchive};
use obc_formats::release::{self, Identification};
use obc_formats::vgagraph::{Font, GraphArchive, Pic, deplanarize};
use std::sync::Arc;

use std::sync::OnceLock;

use obc_core::trig::Tables;
use obc_formats::vswap::{DigiSound, PageFile, Shape};
use obc_formats::{DataDir, FormatError, Result};
use obc_render::palette::Palette;
use obc_render::view3d::Textures;
use obc_render::xbrz::{HiTexture, Metric};
use obc_sim::data::GameData;

/// File offset of the full-screen picture palette in `COUNT.EXE` (docs/re/DATA_FORMATS.md §9).
const PICTURE_PALETTE: usize = 0x2E3B0;
/// File offset of the dark palette (lighting mode 2).
const DARK_PALETTE: usize = 0x2E0B0;
/// First VGAGRAPH chunk holding a picture.
const STARTPICS: usize = 3;

pub struct Assets {
    pub data: DataDir,
    pub release: Identification,
    pub exe: MzExe,
    /// 3D view, sprites, HUD.
    pub gamepal: Palette,
    /// Full-screen pictures (intro, title, menu, help, credits).
    pub picpal: Palette,
    pub pics: Vec<Pic>,
    pub fonts: Vec<Font>,
    pub walls: Vec<Vec<u8>>,
    pub sprites: Vec<Shape>,
    pub maps: MapArchive,
    pub audio: AudioArchive,
    /// `AUDIOMUS.BC` digitized sounds.
    pub digi: Vec<Vec<u8>>,
    /// `GFXTILES.BC` digitized sounds.
    pub vswap_digi: Vec<DigiSound>,
    /// Behavior tables read from `COUNT.EXE`.
    pub game: Arc<GameData>,
    /// Trig tables shared by simulation and renderer.
    pub trig: Arc<Tables>,
    /// Dark palette (generators off), `COUNT.EXE` file 0x2E0B0.
    pub darkpal: Palette,
    /// xBRZ colour metrics over `gamepal`: index 0xFF transparent (walls,
    /// sprites) and fully opaque (floors and ceilings).
    keyed: Metric,
    opaque: Metric,
    /// Upscaled textures at the current factor, built on first use.
    upscaled: Option<Upscaled>,
}

/// One factor's xBRZ textures (ADR-020).
struct Upscaled {
    n: usize,
    walls: Vec<OnceLock<HiTexture>>,
    flats: Vec<OnceLock<HiTexture>>,
    sprites: Vec<OnceLock<HiTexture>>,
}

impl Assets {
    pub fn load(data: DataDir) -> Result<Self> {
        let release = release::identify(&data);
        if !release.is_complete() {
            let missing: Vec<&str> = release
                .checks
                .iter()
                .filter(|c| {
                    c.file.role == release::Role::Required && c.status != release::FileStatus::Ok
                })
                .map(|c| c.file.name)
                .collect();
            return Err(FormatError::Invalid {
                what: "game data",
                msg: format!("missing or unrecognized files: {}", missing.join(", ")),
            });
        }
        let exe = MzExe::parse(data.read("COUNT.EXE")?)?;
        let gamepal = exe.palette(release.release.gamepal_offset)?;
        let picpal = exe.palette(PICTURE_PALETTE)?;
        let graph = GraphArchive::parse(
            &data.read("VGAHEAD.BC")?,
            &data.read("VGADICT.BC")?,
            data.read("VGAGRAPH.BC")?,
        )?;
        let table = graph.pic_table()?;
        let mut pics = Vec::with_capacity(table.len());
        for (i, &(w, h)) in table.iter().enumerate() {
            let chunk = graph.chunk(STARTPICS + i)?.unwrap_or_default();
            pics.push(deplanarize(&chunk, w, h)?);
        }
        let mut fonts = Vec::new();
        for chunk in [1, 2] {
            if let Some(bytes) = graph.chunk(chunk)? {
                fonts.push(Font::parse(&bytes)?);
            }
        }
        let pf = PageFile::parse(data.read("GFXTILES.BC")?)?;
        let walls =
            (0..pf.num_walls()).map(|i| pf.wall(i).map(<[u8]>::to_vec)).collect::<Result<_>>()?;
        let sprites = (0..pf.num_sprites()).map(|i| pf.sprite(i)).collect::<Result<_>>()?;
        let vswap_digi = pf.digi_sounds()?;
        let maps = MapArchive::parse(
            &data.read("MAPHEAD.BC")?,
            data.read("MAPTEMP.BC")?,
            Compression::Rlew,
        )?;
        let audio = AudioArchive::parse(&data.read("AUDIOHED.BC")?, data.read("AUDIOT.BC")?)?;
        let digi = parse_audiomus(&data.read("AUDIOMUS.BC")?)?;
        let game = Arc::new(GameData::load(&exe)?);
        let darkpal = exe.palette(DARK_PALETTE)?;
        let keyed = Metric::new(&gamepal, Some(0xFF));
        let opaque = Metric::new(&gamepal, None);
        Ok(Self {
            data,
            release,
            exe,
            gamepal,
            picpal,
            pics,
            fonts,
            walls,
            sprites,
            maps,
            audio,
            digi,
            vswap_digi,
            game,
            trig: Arc::new(Tables::build()),
            darkpal,
            keyed,
            opaque,
            upscaled: None,
        })
    }

    /// Selects the xBRZ factor for the 3D view's textures (2–6; anything
    /// else turns upscaling off). Changing it drops the textures built so far.
    pub fn set_upscale(&mut self, n: usize) {
        let n = if (2..=6).contains(&n) { n } else { 0 };
        if self.upscaled.as_ref().map_or(0, |u| u.n) == n {
            return;
        }
        let cells = |len: usize| (0..len).map(|_| OnceLock::new()).collect();
        self.upscaled = (n != 0).then(|| Upscaled {
            n,
            walls: cells(self.walls.len()),
            flats: cells(self.walls.len()),
            sprites: cells(self.sprites.len()),
        });
    }

    /// Builds the upscaled versions of the given wall pages and sprites now
    /// (on all cores), so that drawing them later does not stall.
    pub fn prepare_upscaled(&self, walls: &[u16], flats: &[u16], sprites: &[u16]) {
        if self.upscaled.is_none() {
            return;
        }
        let jobs: Vec<(u8, u16)> = walls
            .iter()
            .map(|&p| (0, p))
            .chain(flats.iter().map(|&p| (1, p)))
            .chain(sprites.iter().map(|&p| (2, p)))
            .collect();
        let threads = std::thread::available_parallelism().map_or(4, |n| n.get()).clamp(1, 16);
        let next = std::sync::atomic::AtomicUsize::new(0);
        std::thread::scope(|s| {
            for _ in 0..threads {
                s.spawn(|| {
                    loop {
                        let i = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        let Some(&(kind, id)) = jobs.get(i) else { break };
                        match kind {
                            0 => self.wall_hi(id),
                            1 => self.flat_hi(id),
                            _ => self.sprite_hi(id),
                        };
                    }
                });
            }
        });
    }
}

impl Textures for Assets {
    fn wall(&self, page: u16) -> Option<&[u8]> {
        self.walls.get(page as usize).map(Vec::as_slice)
    }

    fn sprite(&self, index: u16) -> Option<&Shape> {
        self.sprites.get(index as usize)
    }

    fn hires(&self) -> usize {
        self.upscaled.as_ref().map_or(1, |u| u.n)
    }

    fn wall_hi(&self, page: u16) -> Option<&HiTexture> {
        let u = self.upscaled.as_ref()?;
        let src = self.walls.get(usize::from(page))?;
        let cell = u.walls.get(usize::from(page))?;
        Some(cell.get_or_init(|| HiTexture::from_page(src, u.n, &self.keyed, true, false)))
    }

    fn flat_hi(&self, page: u16) -> Option<&HiTexture> {
        let u = self.upscaled.as_ref()?;
        let src = self.walls.get(usize::from(page))?;
        let cell = u.flats.get(usize::from(page))?;
        Some(cell.get_or_init(|| HiTexture::from_page(src, u.n, &self.opaque, true, true)))
    }

    fn sprite_hi(&self, index: u16) -> Option<&HiTexture> {
        let u = self.upscaled.as_ref()?;
        let sh = self.sprites.get(usize::from(index))?;
        let cell = u.sprites.get(usize::from(index))?;
        Some(cell.get_or_init(|| {
            // Transparent texels become 0xFF, as the renderer treats them.
            let px: Vec<u8> =
                sh.pixels.iter().zip(&sh.opaque).map(|(&p, &o)| if o { p } else { 0xFF }).collect();
            HiTexture::from_page(&px, u.n, &self.keyed, false, false)
        }))
    }
}
