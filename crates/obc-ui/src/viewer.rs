//! Developer map viewer: free camera, no collision (MILESTONES M4.8).
//!
//! Uses a provisional tile mapping (plane-0 values 1–136 are walls drawing
//! page `value − 1`) until level-code semantics land (docs/re/LEVEL_CODES.md).

use obc_core::input::{pad, sc};
use obc_render::view3d::{Cell, FloorCeiling, Scene, View3D, ViewConfig};

use crate::assets::Assets;
use crate::present::Presentation;
use crate::screen::{Ctx, Frame, Screen, Transition};

pub struct Viewer {
    slot: usize,
    name: String,
    cells: Vec<Cell>,
    x: f64,
    y: f64,
    angle: f64,
    view: Option<(Presentation, View3D)>,
}

impl Viewer {
    pub fn new(assets: &Assets, slot: usize) -> Self {
        let mut v = Self {
            slot,
            name: String::new(),
            cells: vec![Cell::Empty; 4096],
            x: 32.5,
            y: 32.5,
            angle: 0.0,
            view: None,
        };
        v.load(assets);
        v
    }

    fn load(&mut self, assets: &Assets) {
        let Ok(Some(map)) = assets.maps.map(self.slot) else { return };
        self.name = map.name.clone();
        for ty in 0..64 {
            for tx in 0..64 {
                let v = map.tile(0, tx, ty);
                self.cells[tx * 64 + ty] = if (1..=136).contains(&v) {
                    Cell::Wall { horiz: v - 1, vert: v - 1 }
                } else {
                    Cell::Empty
                };
            }
        }
        // Start in the first open cell near the map center.
        let open: Vec<usize> = (0..4096).filter(|&i| self.cells[i] == Cell::Empty).collect();
        if let Some(&i) = open.iter().min_by_key(|&&i| {
            let (x, y) = ((i / 64) as i32, (i % 64) as i32);
            (x - 32).abs() + (y - 32).abs()
        }) {
            self.x = (i / 64) as f64 + 0.5;
            self.y = (i % 64) as f64 + 0.5;
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
            self.slot = (self.slot + 1) % 61;
            self.load(ctx.assets);
        }
        if input.pressed(sc::PGDN) || input.pad_pressed(pad::LEFT_SHOULDER) {
            self.slot = (self.slot + 60) % 61;
            self.load(ctx.assets);
        }
        let dz = ctx.settings.input.gamepad_deadzone;
        let stick = |v: f32| if v.abs() < dz { 0.0 } else { f64::from(v) };
        let p = &input.pad;
        let run = if input.down(sc::LSHIFT) || p.left_trigger > 0.5 { 2.0 } else { 1.0 };
        let mut fwd = 0.0;
        let mut side = 0.0;
        let mut turn = 0.0;
        if input.down(sc::W) || input.down(sc::UP) {
            fwd += 1.0;
        }
        if input.down(sc::S) || input.down(sc::DOWN) {
            fwd -= 1.0;
        }
        if input.down(sc::A) {
            side -= 1.0;
        }
        if input.down(sc::D) {
            side += 1.0;
        }
        if input.down(sc::LEFT) {
            turn += 1.0;
        }
        if input.down(sc::RIGHT) {
            turn -= 1.0;
        }
        fwd -= stick(p.left_y);
        side += stick(p.left_x);
        turn -= stick(p.right_x) * 1.5;
        self.angle += turn * 3.0
            - f64::from(input.mouse_dx) * 0.2 * f64::from(ctx.settings.input.mouse_sensitivity);
        self.angle = self.angle.rem_euclid(360.0);
        let (s, c) = self.angle.to_radians().sin_cos();
        let speed = 0.06 * run;
        self.x += (c * fwd + s * side) * speed;
        self.y += (-s * fwd + c * side) * speed;
        Transition::None
    }

    fn draw(&mut self, assets: &Assets, out: &mut Frame) {
        let p = out.presentation;
        let (fw, fh) = p.frame_size();
        // The view fills the frame minus a 40-line status area, like the full view size.
        let k = p.scale;
        let vh = (160 - 8) * k;
        let vw = fw - 16 * k;
        if self.view.as_ref().is_none_or(|(vp, _)| *vp != p) {
            let fov = 304 * k;
            let cfg =
                ViewConfig { width: vw & !1, height: vh, fov_width: fov, classic: p.is_classic() };
            self.view = Some((p, View3D::new(cfg)));
        }
        let Some((_, view)) = &mut self.view else { return };
        out.frame.clear(0x7f);
        let scene = Scene {
            cells: &self.cells,
            pwall_pos: 0,
            jamb: (0, 0),
            floor_ceiling: FloorCeiling::Solid { ceiling: 0x1d, floor: 0x19 },
            player_x: (self.x * 65536.0) as i32,
            player_y: (self.y * 65536.0) as i32,
            player_angle: self.angle as i32,
            sprites: &[],
            weapon: None,
        };
        view.render(out.frame, (8 * k) as i32, (4 * k) as i32, &scene, assets);
        let _ = fh;
        out.layer.clear(0xff);
        out.layer_key = Some(0xff);
        if let Some(font) = assets.fonts.get(1) {
            let label = format!(
                "{} {}  ({:.1},{:.1}) {:.0}  PgUp/PgDn",
                self.slot, self.name, self.x, self.y, self.angle
            );
            obc_render::draw2d::draw_text(out.layer, font, label.as_bytes(), 4, 186, 0x0f);
        }
        out.palette = assets.gamepal;
    }
}
