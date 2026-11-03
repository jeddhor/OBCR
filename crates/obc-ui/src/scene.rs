//! Builds renderer input from the simulation (`World` → `Scene`).

use obc_render::view3d::{DoorView, FloorParams, SpriteRef, WeaponView};
use obc_sim::data::LevelTables;
use obc_sim::world::{World, class, dir};

/// Per-frame buffers the scene borrows.
#[derive(Default)]
pub struct SceneBuffers {
    pub doors: Vec<DoorView>,
    pub sprites: Vec<SpriteRef>,
}

impl SceneBuffers {
    /// Fills doors and sprites from the world. Statics come first, then actors
    /// in list order after the player (the original's `vislist` order).
    pub fn fill(&mut self, w: &World) {
        self.doors.clear();
        self.doors.extend(w.doors.iter().map(|d| DoorView { position: d.position, page: d.kind }));
        self.sprites.clear();
        for s in &w.statics {
            if s.shape == -1 {
                continue;
            }
            self.sprites.push(SpriteRef {
                x: (i32::from(s.tilex) << 16) + 0x8000,
                y: (i32::from(s.tiley) << 16) + 0x8000,
                tile_x: s.tilex,
                tile_y: s.tiley,
                shape: s.shape as u16,
                is_static: true,
                rotate: 0,
                facing: 0,
            });
        }
        let mut cur = w.plr().next;
        while let Some(id) = cur {
            let o = w.obj(id);
            cur = o.next;
            let Some(st) = w.data.state(o.state) else { continue };
            if st.shape == 0 {
                continue;
            }
            let shape = if st.shape == -1 { o.temp1 } else { st.shape };
            let facing = if o.obclass == class::GRENADE {
                i32::from(o.angle)
            } else {
                dir::DIRANGLE[usize::from(o.dir.min(8))]
            };
            self.sprites.push(SpriteRef {
                x: o.x,
                y: o.y,
                tile_x: o.tilex as u8,
                tile_y: o.tiley as u8,
                shape: shape as u16,
                is_static: false,
                rotate: st.rotate as u8,
                facing,
            });
        }
    }
}

/// Floor/ceiling parameters for a level (docs/re/RENDERER.md §3.1).
pub fn floor_params(levels: &LevelTables, w: &World, mode: u8) -> FloorParams {
    let m = LevelTables::index(w.gamestate.mapon);
    let r = usize::from(levels.rindex.get(m).copied().unwrap_or(0)).min(levels.ramps.len() - 1);
    let f = usize::from(levels.findex.get(m).copied().unwrap_or(0)).min(levels.ramps.len() - 1);
    FloorParams {
        mode,
        cc: levels.rclr.get(m).copied().unwrap_or(0),
        fc: levels.fclr.get(m).copied().unwrap_or(0),
        rc: levels.ramps[r].1,
        rlim: levels.ramps[r].0,
        rf: levels.ramps[f].0,
        fl: levels.ramps[f].1,
        quad: if (0..=4).contains(&w.gamestate.mapon) { 0x8400 } else { 0x7600 },
        sets: w.floor_sets,
    }
}

/// Hand-weapon sprites per weapon: left and right halves (DS:0x0D2A, DS:0x0D34).
const WEAPON_LEFT: [u16; 5] = [544, 552, 560, 568, 576];
const WEAPON_RIGHT: [u16; 5] = [584, 592, 600, 608, 616];
/// `bobx[]` (DS:0x31DC) and `bobframe[]` (DS:0x31FC).
const BOBX: [i32; 16] = [0, 1, 2, 3, 4, 3, 2, 1, 0, -1, -2, -3, -4, -3, -2, -1];
const BOBFRAME: [u16; 16] = [7, 7, 6, 5, 4, 5, 6, 7, 7, 7, 6, 5, 4, 5, 6, 7];

/// The weapon to draw (`DrawPlayerWeapon` 1b4a:1013), bobbing by *walkcount*.
pub fn weapon_view(w: &World) -> Option<WeaponView> {
    let g = &w.gamestate;
    if !(0..5).contains(&g.weapon) {
        return None;
    }
    let b = (w.ply.walkcount & 15) as usize;
    let idle = !w.buttonheld[0] && g.attackframe == 0;
    let frame = if idle { BOBFRAME[b] } else { g.weaponframe.max(0) as u16 };
    let i = g.weapon as usize;
    Some(WeaponView { left: WEAPON_LEFT[i] + frame, right: WEAPON_RIGHT[i] + frame, bob: BOBX[b] })
}
