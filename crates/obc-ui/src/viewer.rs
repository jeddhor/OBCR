//! Developer map viewer: free camera over a level set up by the simulation
//! (MILESTONES M4.8). PgUp/PgDn change maps, F5/F6 floor modes.

use obc_core::input::{pad, sc};
use obc_render::Framebuffer;
use obc_render::view3d::{Scene, View3D, ViewConfig, build_shade_table};
use obc_sim::world::World;

use crate::assets::Assets;
use crate::present::Presentation;
use crate::scene::{SceneBuffers, floor_params};
use crate::screen::{Ctx, Frame, Screen, Transition};

/// Classic view size used by the viewer and the game (`viewsize`, default 18).
pub const VIEWSIZE: u16 = 18;

/// Where the 3D view sits in a frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Layout {
    pub x: i32,
    pub y: i32,
    pub cfg: ViewConfig,
}

/// The original layout (`SetViewSize`): 16n × 8n centred in the top 160 lines,
/// widened sideways for widescreen with the same border thickness.
pub fn layout(p: Presentation, viewsize: u16) -> Layout {
    let k = p.scale;
    let (fw, _) = p.frame_size();
    let w0 = usize::from(viewsize) * 16;
    let h0 = usize::from(viewsize) * 8;
    let margin = (320 - w0) / 2;
    let width = (fw - 2 * margin * k) & !1;
    Layout {
        x: (margin * k) as i32,
        y: (((160 - h0) / 2) * k) as i32,
        cfg: ViewConfig { viewsize, k, width },
    }
}

/// Fills a frame with the 8×8 background tile (pic 0) at the presentation scale.
pub fn tile_background(frame: &mut Framebuffer, assets: &Assets, k: usize) {
    let Some(tile) = assets.pics.first() else { return };
    for y in 0..frame.height {
        for x in 0..frame.width {
            let (tx, ty) = ((x / k) % 8, (y / k) % 8);
            frame.pixels[y * frame.width + x] = tile.pixels[ty * 8 + tx];
        }
    }
}

pub struct Viewer {
    slot: usize,
    world: World,
    x: f64,
    y: f64,
    angle: f64,
    floor_mode: u8,
    view: Option<(Layout, View3D)>,
    buffers: SceneBuffers,
}

impl Viewer {
    pub fn new(assets: &Assets, slot: usize) -> Self {
        let mut v = Self {
            slot,
            world: World::new(assets.game.clone(), assets.trig.clone()),
            x: 32.5,
            y: 32.5,
            angle: 0.0,
            floor_mode: 2,
            view: None,
            buffers: SceneBuffers::default(),
        };
        v.load(assets);
        v
    }

    fn load(&mut self, assets: &Assets) {
        let Ok(Some(map)) = assets.maps.map(self.slot) else { return };
        self.world.setup_game_level(&map, self.slot as i16);
        // Start at the player start if present, else the open cell nearest the centre.
        let start = (0..4096).find(|&i| (19..=22).contains(&map.planes[1][i]));
        let open = |i: usize| self.world.tilemap[(i % 64) * 64 + i / 64] == 0;
        let i = start.or_else(|| {
            (0..4096)
                .filter(|&i| open(i))
                .min_by_key(|&i| (i % 64).abs_diff(32) + (i / 64).abs_diff(32))
        });
        if let Some(i) = i {
            self.x = (i % 64) as f64 + 0.5;
            self.y = (i / 64) as f64 + 0.5;
            if let Some(d) = start.map(|i| map.planes[1][i] - 19) {
                self.angle = f64::from((1 - i32::from(d)) * 90).rem_euclid(360.0);
            }
        }
    }
}

impl Screen for Viewer {
    fn tick(&mut self, ctx: &mut Ctx) -> Transition {
        let input = ctx.input;
        ctx.requests.relative_mouse = Some(true);
        if input.pressed(sc::ESCAPE) || input.pad_pressed(pad::BACK) {
            return Transition::Quit;
        }
        if input.pressed(sc::PGUP) || input.pad_pressed(pad::RIGHT_SHOULDER) {
            self.slot = (self.slot + 1) % 60;
            self.load(ctx.assets);
        }
        if input.pressed(sc::PGDN) || input.pad_pressed(pad::LEFT_SHOULDER) {
            self.slot = (self.slot + 59) % 60;
            self.load(ctx.assets);
        }
        if input.pressed(sc::F1 + 4) {
            self.floor_mode = 0;
        }
        if input.pressed(sc::F1 + 5) {
            self.floor_mode = 2;
        }
        let dz = ctx.settings.input.gamepad_deadzone;
        let stick = |v: f32| if v.abs() < dz { 0.0 } else { f64::from(v) };
        let p = &input.pad;
        let run = if input.down(sc::LSHIFT) || p.left_trigger > 0.5 { 2.0 } else { 1.0 };
        let mut fwd = -stick(p.left_y);
        let mut side = stick(p.left_x);
        let mut turn = -stick(p.right_x) * 1.5;
        for (key, f, s, t) in [
            (sc::W, 1.0, 0.0, 0.0),
            (sc::UP, 1.0, 0.0, 0.0),
            (sc::S, -1.0, 0.0, 0.0),
            (sc::DOWN, -1.0, 0.0, 0.0),
            (sc::A, 0.0, -1.0, 0.0),
            (sc::D, 0.0, 1.0, 0.0),
            (sc::LEFT, 0.0, 0.0, 1.0),
            (sc::RIGHT, 0.0, 0.0, -1.0),
        ] {
            if input.down(key) {
                fwd += f;
                side += s;
                turn += t;
            }
        }
        self.angle += turn * 3.0
            - f64::from(input.mouse_dx) * 0.2 * f64::from(ctx.settings.input.mouse_sensitivity);
        self.angle = self.angle.rem_euclid(360.0);
        let (s, c) = self.angle.to_radians().sin_cos();
        let speed = 0.06 * run;
        self.x += (c * fwd + s * side) * speed;
        self.y += (-s * fwd + c * side) * speed;
        self.world.move_anim_walls();
        Transition::None
    }

    fn draw(&mut self, assets: &Assets, out: &mut Frame) {
        let p = out.presentation;
        let l = layout(p, VIEWSIZE);
        if self.view.as_ref().is_none_or(|(vl, _)| *vl != l) {
            self.view = Some((l, View3D::new(l.cfg)));
        }
        tile_background(out.frame, assets, p.scale);
        self.buffers.fill(&self.world);
        let table = build_shade_table(self.world.dark);
        let scene = Scene {
            tiles: &self.world.tilemap,
            info: &self.world.plane1,
            plane0: &self.world.plane0,
            doors: &self.buffers.doors,
            pwall_pos: self.world.pwall.pos,
            floor: floor_params(&assets.game.levels, &self.world, self.floor_mode),
            shade_table: &table,
            shade_bias: 0,
            player_x: (self.x * 65536.0) as i32,
            player_y: (self.y * 65536.0) as i32,
            player_angle: self.angle as i32,
            sprites: &self.buffers.sprites,
            weapon: None,
        };
        if let Some((_, view)) = &mut self.view {
            view.render(out.frame, l.x, l.y, &scene, assets);
        }
        out.layer.clear(0xFF);
        out.layer_key = Some(0xFF);
        if let Some(bar) = assets.pics.get(29) {
            obc_render::draw2d::draw_pic(out.layer, bar, 0, 160);
        }
        if let Ok(Some(name)) = assets.maps.name(self.slot)
            && let Some(font) = assets.fonts.first()
        {
            let label = format!("{} {}", self.slot, name);
            obc_render::draw2d::draw_text(out.layer, font, label.as_bytes(), 2, 2, 15);
        }
        out.palette = assets.gamepal;
    }
}
