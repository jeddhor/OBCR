//! Converting a mod's images to the game palette
//! (docs/MOD_ASSETS_AND_ARENAS.md §2.4).
//!
//! One rule, shared with OBCEdit's preview (`obcedit_core.artimport`) and
//! checked by both against `docs/test-vectors/quantize.json`: alpha below 128
//! is transparent (index 255, sprites only; wall pixels are always opaque),
//! anything else is the palette entry 0-254 nearest in squared RGB distance,
//! the lowest index on a tie.

use std::collections::HashMap;

use crate::palette::Palette;

/// The transparent index of sprites and see-through walls.
pub const TRANSPARENT: u8 = 0xFF;

/// Nearest-colour lookup into one palette, with a cache of colours seen.
#[derive(Debug, Clone)]
pub struct Quantizer {
    palette: Palette,
    cache: HashMap<[u8; 3], u8>,
}

impl Quantizer {
    pub fn new(palette: &Palette) -> Self {
        Self { palette: *palette, cache: HashMap::new() }
    }

    /// The entry 0-254 nearest to `rgb`.
    pub fn nearest(&mut self, rgb: [u8; 3]) -> u8 {
        if let Some(&i) = self.cache.get(&rgb) {
            return i;
        }
        let mut best = (u32::MAX, 0u8);
        for (i, c) in self.palette[..255].iter().enumerate() {
            let d: u32 = (0..3).map(|k| (i32::from(c[k]) - i32::from(rgb[k])).pow(2) as u32).sum();
            if d < best.0 {
                best = (d, i as u8);
            }
        }
        self.cache.insert(rgb, best.1);
        best.1
    }

    /// One pixel of a sprite: transparent below alpha 128.
    pub fn sprite_pixel(&mut self, rgba: [u8; 4]) -> u8 {
        if rgba[3] < 128 { TRANSPARENT } else { self.nearest([rgba[0], rgba[1], rgba[2]]) }
    }

    /// One pixel of a wall: alpha is ignored.
    pub fn wall_pixel(&mut self, rgba: [u8; 4]) -> u8 {
        self.nearest([rgba[0], rgba[1], rgba[2]])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_the_shared_vectors() {
        let doc: serde_json::Value =
            serde_json::from_str(include_str!("../../../docs/test-vectors/quantize.json")).unwrap();
        let mut pal: Palette = [[0; 3]; 256];
        for (i, c) in doc["palette"].as_array().unwrap().iter().enumerate() {
            for k in 0..3 {
                pal[i][k] = c[k].as_u64().unwrap() as u8;
            }
        }
        let mut q = Quantizer::new(&pal);
        let cases = doc["cases"].as_array().unwrap();
        assert!(cases.len() >= 100);
        for case in cases {
            let px: Vec<u8> = case["rgba"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_u64().unwrap() as u8)
                .collect();
            let rgba = [px[0], px[1], px[2], px[3]];
            assert_eq!(
                u64::from(q.wall_pixel(rgba)),
                case["wall"].as_u64().unwrap(),
                "wall {rgba:?}"
            );
            assert_eq!(
                u64::from(q.sprite_pixel(rgba)),
                case["sprite"].as_u64().unwrap(),
                "sprite {rgba:?}"
            );
        }
    }
}
