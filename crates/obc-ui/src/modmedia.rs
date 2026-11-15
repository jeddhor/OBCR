//! A mod's own art and sounds, converted for the game when a mod game starts
//! (docs/MOD_ASSETS_AND_ARENAS.md §3.2-3.4).
//!
//! The base [`Assets`] are never changed: [`Layered`] draws a replaced wall
//! page or sprite from the mod and everything else from the game, and is used
//! only when a mod has art, so the original path is untouched. The xBRZ
//! versions of replaced textures are built lazily per factor, like the base
//! ones.

use std::collections::HashMap;
use std::sync::{Arc, OnceLock};

use obc_formats::modassets::{self, Image};
use obc_formats::modpack::Package;
use obc_formats::vswap::{SHAPE_SIZE, Shape};
use obc_render::palette::Palette;
use obc_render::quantize::{Quantizer, TRANSPARENT};
use obc_render::view3d::Textures;
use obc_render::xbrz::HiTexture;

use crate::assets::Assets;

/// xBRZ factors are 2-6.
const FACTORS: usize = 7;

#[derive(Default)]
struct HiCache {
    walls: HashMap<u16, OnceLock<HiTexture>>,
    flats: HashMap<u16, OnceLock<HiTexture>>,
    sprites: HashMap<u16, OnceLock<HiTexture>>,
}

/// A mod's converted assets.
pub struct ModMedia {
    /// Replaced wall pages: 4096 column-major palette indices.
    pub walls: HashMap<u16, Vec<u8>>,
    pub sprites: HashMap<u16, Shape>,
    /// Replaced digitized sounds: id, 8-bit unsigned mono, rate.
    pub sounds: Vec<(u16, Arc<[u8]>, u32)>,
    package: Arc<Package>,
    hi: [OnceLock<HiCache>; FACTORS],
}

impl std::fmt::Debug for ModMedia {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ModMedia")
            .field("walls", &self.walls.len())
            .field("sprites", &self.sprites.len())
            .field("sounds", &self.sounds.len())
            .finish()
    }
}

/// A row-major image as column-major palette indices.
fn columns(img: &Image, mut pixel: impl FnMut([u8; 4]) -> u8) -> Vec<u8> {
    let mut out = vec![0u8; SHAPE_SIZE * SHAPE_SIZE];
    for x in 0..SHAPE_SIZE {
        for y in 0..SHAPE_SIZE {
            out[x * SHAPE_SIZE + y] = pixel(img.at(x, y));
        }
    }
    out
}

impl ModMedia {
    /// Decodes and converts the package's art and sounds to the game palette
    /// and sample format. A file that fails (the mod check already refused
    /// such mods) is left as the original's, with a message.
    pub fn load(package: Arc<Package>, palette: &Palette) -> Self {
        let mut q = Quantizer::new(palette);
        let a = &package.assets;
        let image = |path: &str| {
            let r = package.read(path).ok_or_else(|| "cannot be read".to_owned());
            r.and_then(|b| modassets::decode_png(&b)).map_err(|e| eprintln!("{path}: {e}")).ok()
        };
        let walls = a
            .walls
            .iter()
            .filter_map(|(&n, p)| image(p).map(|img| (n, columns(&img, |c| q.wall_pixel(c)))))
            .collect();
        let sprites = a
            .sprites
            .iter()
            .filter_map(|(&n, p)| {
                image(p).map(|img| {
                    (n, Shape::from_indexed(&columns(&img, |c| q.sprite_pixel(c)), TRANSPARENT))
                })
            })
            .collect();
        let sounds = a
            .sounds
            .iter()
            .filter_map(|(&n, p)| {
                let s = package.read(p).ok_or_else(|| "cannot be read".to_owned());
                match s.and_then(|b| modassets::decode_wav(&b)) {
                    Ok(s) => Some((n, Arc::from(s.samples), s.rate)),
                    Err(e) => {
                        eprintln!("{p}: {e}");
                        None
                    }
                }
            })
            .collect();
        Self { walls, sprites, sounds, package, hi: Default::default() }
    }

    /// Whether the mod replaces any art (else [`Layered`] is not needed).
    pub fn has_art(&self) -> bool {
        !self.walls.is_empty() || !self.sprites.is_empty()
    }

    /// A music file named in the manifest.
    pub fn music(&self, name: &str) -> Option<Vec<u8>> {
        self.package.music(name)
    }

    fn hi(&self, n: usize) -> Option<&HiCache> {
        let cell = self.hi.get(n)?;
        Some(cell.get_or_init(|| HiCache {
            walls: self.walls.keys().map(|&k| (k, OnceLock::new())).collect(),
            flats: self.walls.keys().map(|&k| (k, OnceLock::new())).collect(),
            sprites: self.sprites.keys().map(|&k| (k, OnceLock::new())).collect(),
        }))
    }
}

/// The game's textures with a mod's replacements on top.
pub struct Layered<'a> {
    pub base: &'a Assets,
    pub media: &'a ModMedia,
}

impl Textures for Layered<'_> {
    fn wall(&self, page: u16) -> Option<&[u8]> {
        match self.media.walls.get(&page) {
            Some(w) => Some(w),
            None => self.base.wall(page),
        }
    }

    fn sprite(&self, index: u16) -> Option<&Shape> {
        self.media.sprites.get(&index).or_else(|| self.base.sprite(index))
    }

    fn hires(&self) -> usize {
        self.base.hires()
    }

    fn wall_hi(&self, page: u16) -> Option<&HiTexture> {
        let Some(src) = self.media.walls.get(&page) else { return self.base.wall_hi(page) };
        let n = self.base.hires();
        let cell = self.media.hi(n)?.walls.get(&page)?;
        Some(
            cell.get_or_init(|| {
                HiTexture::from_page(src, n, self.base.keyed_metric(), true, false)
            }),
        )
    }

    fn flat_hi(&self, page: u16) -> Option<&HiTexture> {
        let Some(src) = self.media.walls.get(&page) else { return self.base.flat_hi(page) };
        let n = self.base.hires();
        let cell = self.media.hi(n)?.flats.get(&page)?;
        Some(
            cell.get_or_init(|| {
                HiTexture::from_page(src, n, self.base.opaque_metric(), true, true)
            }),
        )
    }

    fn sprite_hi(&self, index: u16) -> Option<&HiTexture> {
        let Some(sh) = self.media.sprites.get(&index) else { return self.base.sprite_hi(index) };
        let n = self.base.hires();
        let cell = self.media.hi(n)?.sprites.get(&index)?;
        Some(cell.get_or_init(|| {
            let px: Vec<u8> =
                sh.pixels.iter().zip(&sh.opaque).map(|(&p, &o)| if o { p } else { 0xFF }).collect();
            HiTexture::from_page(&px, n, self.base.keyed_metric(), false, false)
        }))
    }
}
