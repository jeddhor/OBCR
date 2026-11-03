//! Everything loaded from the original data at startup.

use obc_formats::audio::{AudioArchive, parse_audiomus};
use obc_formats::exe::MzExe;
use obc_formats::maps::{Compression, MapArchive};
use obc_formats::release::{self, Identification};
use obc_formats::vgagraph::{Font, GraphArchive, Pic, deplanarize};
use obc_formats::vswap::{DigiSound, PageFile, Shape};
use obc_formats::{DataDir, FormatError, Result};
use obc_render::palette::Palette;
use obc_render::view3d::Textures;

/// File offset of the full-screen picture palette in `COUNT.EXE` (docs/re/DATA_FORMATS.md §9).
const PICTURE_PALETTE: usize = 0x2E3B0;
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
        })
    }
}

impl Textures for Assets {
    fn wall(&self, page: u16) -> Option<&[u8]> {
        self.walls.get(page as usize).map(Vec::as_slice)
    }

    fn sprite(&self, index: u16) -> Option<&Shape> {
        self.sprites.get(index as usize)
    }
}
