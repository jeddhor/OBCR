//! The 3D view (`ThreeDRefresh` ghidra 1b4a:129c; docs/re/RENDERER.md).
//!
//! Order: background gradient → ray pass A (`spotvis`) → textured floor and
//! ceiling quads → pass B (walls, doors, push walls) → sprites → pass C
//! (see-through and thin walls) → sprites again → the two-part weapon.
//!
//! Geometry is Wolf3D's (`WOLFSRC/WL_DRAW.C`, `WL_DR_A.ASM`, `WL_SCALE.C`).
//! The view renders at `k` times the original resolution; with `k = 1` and
//! the original width it reproduces the original integer pipeline.

use obc_core::fixed::fixed_by_frac;
use obc_core::trig::{FINEANGLES, Tables, radtoint};
use obc_formats::vswap::Shape;

use crate::framebuffer::Framebuffer;

const TILEGLOBAL: i32 = 1 << 16;
const FOCALLENGTH: i32 = 0x5700;
const MINDIST: i32 = 0x5800;
const VIEWGLOBAL: i64 = 0x10000;
const ACTORSIZE: i32 = 0x4000;
const DEG90: i32 = 900;
const DEG180: i32 = 1800;
const DEG270: i32 = 2700;
const DEG360: i32 = 3600;
/// `vislist` entries.
const MAXVISIBLE: usize = 75;
/// Door-frame page (`PMSpriteStart − 6`).
const DOOR_FRAME_PAGE: u16 = 130;

/// A door as the renderer needs it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct DoorView {
    /// Leading-edge position: 0 closed … 0xFFFF open.
    pub position: u16,
    pub page: u16,
}

/// Background and floor/ceiling parameters (docs/re/RENDERER.md §3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FloorParams {
    /// 0 = ramp gradient only; otherwise colour gradient plus textured tiles.
    pub mode: u8,
    /// Colour mode: ceiling and floor colours (`Rclr`, `Fclr`).
    pub cc: u8,
    pub fc: u8,
    /// Ramp mode: ceiling start/limit, floor start/limit.
    pub rc: u8,
    pub rlim: u8,
    pub rf: u8,
    pub fl: u8,
    /// Half size of a floor quad (`DS:0x09E2`): 0x7600, or 0x8400 in the sewers.
    pub quad: i32,
    /// Texture sets: `(floor page + 1, ceiling page + 1)`.
    pub sets: [(u8, u8); 10],
}

/// A drawable object.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SpriteRef {
    pub x: i32,
    pub y: i32,
    pub tile_x: u8,
    pub tile_y: u8,
    pub shape: u16,
    pub is_static: bool,
    /// 0 none, 1 eight rotations, 2 pain frame (+0/+4).
    pub rotate: u8,
    /// Facing angle in degrees (`dirangle[dir]`, or `angle` for grenades).
    pub facing: i32,
}

/// The two-part player weapon.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WeaponView {
    pub left: u16,
    pub right: u16,
    /// Horizontal bob offset in original pixels (`bobx[]`).
    pub bob: i32,
}

/// What to draw.
pub struct Scene<'a> {
    /// `tilemap`, index `x * 64 + y`.
    pub tiles: &'a [u8],
    /// Plane 1, index `y * 64 + x`.
    pub info: &'a [u16],
    /// Plane 0, index `y * 64 + x`.
    pub plane0: &'a [u16],
    pub doors: &'a [DoorView],
    pub pwall_pos: u16,
    pub floor: FloorParams,
    /// Distance-shade table (`DS:0xC2CA`).
    pub shade_table: &'a [u8; 256],
    /// `2·muzzle + brightness` added to the shade index.
    pub shade_bias: i32,
    pub player_x: i32,
    pub player_y: i32,
    pub player_angle: i32,
    pub sprites: &'a [SpriteRef],
    pub weapon: Option<WeaponView>,
}

/// Source of wall and sprite pixels.
pub trait Textures {
    /// 4096 column-major palette indices.
    fn wall(&self, page: u16) -> Option<&[u8]>;
    fn sprite(&self, index: u16) -> Option<&Shape>;
}

/// View geometry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ViewConfig {
    /// Original view size `n` (view is 16n × 8n original pixels).
    pub viewsize: u16,
    /// Resolution multiple.
    pub k: usize,
    /// Rendered width in pixels; wider than `16n·k` for widescreen (Hor+).
    pub width: usize,
}

impl ViewConfig {
    pub fn classic(viewsize: u16) -> Self {
        Self { viewsize, k: 1, width: usize::from(viewsize) * 16 }
    }

    pub fn base_width(&self) -> usize {
        usize::from(self.viewsize) * 16
    }

    pub fn height(&self) -> usize {
        usize::from(self.viewsize) * 8 * self.k
    }

    pub fn is_classic(&self) -> bool {
        self.k == 1 && self.width == self.base_width()
    }
}

/// `BuildCompScale`: screen rows covered by each texel, and per-column widths.
#[derive(Debug, Clone)]
struct Scaler {
    rows: [(i32, i32); 64],
    width: [i32; 65],
}

impl Scaler {
    fn build(height: i32, view_height: i32) -> Self {
        let step = (i64::from(height) << 16) / 64;
        let toppix = (view_height - height) / 2;
        let mut fix: i64 = 0;
        let mut rows = [(0, 0); 64];
        let mut width = [0; 65];
        for src in 0..=64usize {
            let start = (fix >> 16) as i32;
            fix += step;
            let end = (fix >> 16) as i32;
            width[src] = (end - start).max(0);
            if src < 64 {
                rows[src] = (start + toppix, end + toppix);
            }
        }
        Self { rows, width }
    }
}

/// Shades a wall or sprite texel: `None` if transparent (0xFF). Full-bright
/// indices (0–15, 224–255) are unshaded; a ramp texel shaded to (or at) its
/// row base becomes black (docs/re/RENDERER.md §4.2).
#[inline]
fn shade_texel(t: u8, shade: u8) -> Option<u8> {
    if t == 0xFF {
        return None;
    }
    if !(16..=223).contains(&t) {
        return Some(t);
    }
    let s = t.wrapping_sub(shade);
    Some(if s > (t & 0xF0) { s } else { 0 })
}

/// The 256-entry distance-shade table (`30d6:11b6`): `dark` uses the sewer curve.
pub fn build_shade_table(dark: bool) -> [u8; 256] {
    let g: f64 = if dark { 5.5 } else { 2.5 };
    let mut t = [0u8; 256];
    t[0] = 16;
    for (i, v) in t.iter_mut().enumerate().skip(1) {
        let s = (16.0 / (i as f64 / g).sqrt() - 1.0) as f32;
        *v = s.min(16.0) as u8;
    }
    t
}

#[derive(Debug, Clone, Copy)]
enum Hit {
    Wall {
        vertical: bool,
        x: i32,
        y: i32,
        xi: i32,
        yi: i32,
        page: u16,
    },
    /// Centre-plane surface: doors and thin walls. `offset` is subtracted from the intercept.
    Thin {
        vertical: bool,
        x: i32,
        y: i32,
        xi: i32,
        yi: i32,
        page: u16,
        offset: u16,
    },
    PushWall {
        vertical: bool,
        x: i32,
        y: i32,
        xi: i32,
        yi: i32,
        page: u16,
    },
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Pass {
    /// Visibility only.
    A,
    /// Solid walls.
    B,
    /// See-through and thin walls.
    C,
}

enum CellResult {
    Pass,
    Stop,
    Hit(Hit),
}

struct Ray {
    viewx: i32,
    viewy: i32,
    viewsin: i32,
    viewcos: i32,
}

struct Post {
    postx: usize,
    width: usize,
    page: u16,
    column: u16,
    lastside: i32,
    key: i32,
}

/// Projection and per-frame working storage.
pub struct View3D {
    cfg: ViewConfig,
    tables: Tables,
    w: usize,
    h: usize,
    centerx: i32,
    scale: i64,
    heightnumerator: i64,
    pixelangle: Vec<i32>,
    scaler_for: Vec<usize>,
    scalers: Vec<Scaler>,
    maxscale: i32,
    wallheight: Vec<i32>,
    see_through: usize,
    /// Tiles a ray passed through this frame.
    pub spotvis: Vec<bool>,
}

impl View3D {
    pub fn new(cfg: ViewConfig) -> Self {
        let mut v = Self {
            cfg,
            tables: Tables::build(),
            w: cfg.width,
            h: cfg.height(),
            centerx: 0,
            scale: 0,
            heightnumerator: 0,
            pixelangle: Vec::new(),
            scaler_for: Vec::new(),
            scalers: Vec::new(),
            maxscale: 0,
            wallheight: vec![0; cfg.width],
            see_through: 0,
            spotvis: vec![false; 4096],
        };
        v.calc_projection();
        v.setup_scaling();
        v
    }

    pub fn config(&self) -> ViewConfig {
        self.cfg
    }

    /// `CalcProjection(FOCALLENGTH)` at k× resolution (Hor+ for extra width).
    fn calc_projection(&mut self) {
        let fov = (self.cfg.base_width() * self.cfg.k) as i64;
        let facedist = f64::from(FOCALLENGTH + MINDIST);
        self.centerx = (self.w / 2) as i32 - 1;
        self.scale = ((fov / 2) as f64 * facedist / (VIEWGLOBAL / 2) as f64) as i64;
        self.heightnumerator = (i64::from(TILEGLOBAL) * self.scale) >> 6;
        let halfview = self.w / 2;
        let r = radtoint();
        self.pixelangle = vec![0; self.w];
        for i in 0..halfview {
            let tang = (i as i64 * VIEWGLOBAL / fov) as f64 / facedist;
            let intang = (f64::from(tang.atan() as f32) * r) as i32;
            self.pixelangle[halfview - 1 - i] = intang;
            self.pixelangle[halfview + i] = -intang;
        }
    }

    /// `SetupScaling(viewwidth·1.5)`.
    fn setup_scaling(&mut self) {
        let vh = self.h as i32;
        let classic = self.cfg.is_classic();
        let maxscaleheight = if classic { (self.w as i32 * 3 / 2) / 2 } else { vh * 8 };
        self.maxscale = maxscaleheight - 1;
        let stepbytwo = vh / 2;
        self.scalers.clear();
        self.scaler_for = vec![0; maxscaleheight as usize + 3];
        let mut i = 1;
        while i <= maxscaleheight {
            self.scalers.push(Scaler::build(i * 2, vh));
            let idx = self.scalers.len() - 1;
            self.scaler_for[i as usize] = idx;
            if classic && i >= stepbytwo {
                for k in 1..=2 {
                    if let Some(s) = self.scaler_for.get_mut((i + k) as usize) {
                        *s = idx;
                    }
                }
                i += 2;
            }
            i += 1;
        }
        self.scaler_for[0] = self.scaler_for[1];
    }

    fn scaler(&self, index: i32) -> &Scaler {
        let i = (index.max(0) as usize).min(self.scaler_for.len() - 1);
        &self.scalers[self.scaler_for[i]]
    }

    fn k(&self) -> i32 {
        self.cfg.k as i32
    }

    /// Shade from a height in k-scaled eighth-pixels.
    fn shade_for(&self, height: i32, scene: &Scene) -> u8 {
        let idx = (height / self.k() / 2 + scene.shade_bias).min(255);
        if idx < 0 { scene.shade_table[0] } else { scene.shade_table[idx as usize] }
    }

    /// Draws the view with its top-left at (`x0`, `y0`).
    pub fn render(
        &mut self,
        fb: &mut Framebuffer,
        x0: i32,
        y0: i32,
        scene: &Scene,
        tex: &dyn Textures,
    ) {
        let ray = self.view_ray(scene);
        self.background(fb, x0, y0, &scene.floor);
        self.spotvis.fill(false);
        self.trace_pass(fb, x0, y0, scene, tex, &ray, Pass::A);
        if scene.floor.mode != 0 {
            self.floor_tiles(fb, x0, y0, scene, tex, &ray);
        }
        self.trace_pass(fb, x0, y0, scene, tex, &ray, Pass::B);
        let vis = self.visible_sprites(scene, &ray);
        self.draw_sprites(fb, x0, y0, &vis, scene, tex);
        if self.see_through > 0 {
            self.trace_pass(fb, x0, y0, scene, tex, &ray, Pass::C);
            self.draw_sprites(fb, x0, y0, &vis, scene, tex);
        }
        if let Some(w) = scene.weapon {
            self.draw_weapon(fb, x0, y0, w, tex);
        }
    }

    fn view_ray(&self, scene: &Scene) -> Ray {
        let viewangle = scene.player_angle.rem_euclid(360) as usize;
        let viewsin = self.tables.sin(viewangle);
        let viewcos = self.tables.cos(viewangle);
        Ray {
            viewx: scene.player_x - fixed_by_frac(FOCALLENGTH, viewcos),
            viewy: scene.player_y + fixed_by_frac(FOCALLENGTH, viewsin),
            viewsin,
            viewcos,
        }
    }

    /// Gradient fills (`1b4a:0c72` colour mode, `1b4a:0bc9` ramp mode); the
    /// two bytes alternate in 4-pixel groups and swap every row (dither).
    fn background(&self, fb: &mut Framebuffer, x0: i32, y0: i32, fp: &FloorParams) {
        let k = self.cfg.k;
        let rows = self.h / 2 / k;
        let w = self.w;
        let put_row = |fb: &mut Framebuffer, row: usize, lo: u8, hi: u8| {
            for s in 0..k {
                for x in 0..w {
                    let c = if (x / (4 * k)) & 1 == 0 { lo } else { hi };
                    fb.put(x0 + x as i32, y0 + (row * k + s) as i32, c);
                }
            }
        };
        let s8 = |v: u8| v as i8;
        let half = rows;
        if fp.mode != 0 {
            let clim = (fp.cc & 0xF0).wrapping_add(1);
            let flim = (fp.fc & 0xF0).wrapping_add(15);
            let (mut ah, mut al, mut n) = (fp.cc, fp.cc.wrapping_sub(1), 0);
            for r in 0..rows {
                put_row(fb, r, al, ah);
                std::mem::swap(&mut ah, &mut al);
                if s8(al) > s8(clim) {
                    n += 1;
                    if n >= 10 {
                        n = 0;
                        ah = ah.wrapping_sub(1);
                        al = al.wrapping_sub(1);
                    }
                }
            }
            let (mut ah, mut al, mut n) = (fp.fc, fp.fc.wrapping_sub(1), 0);
            for r in 0..rows {
                put_row(fb, half + r, al, ah);
                std::mem::swap(&mut ah, &mut al);
                if s8(al) < s8(flim) {
                    n += 1;
                    if n >= 20 {
                        n = 0;
                        ah = ah.wrapping_add(1);
                        al = al.wrapping_add(1);
                    }
                }
            }
        } else {
            let (mut ah, mut al, mut n) = (fp.rc, fp.rc.wrapping_sub(1), 0);
            for r in 0..rows {
                put_row(fb, r, al, ah);
                n += 1;
                std::mem::swap(&mut ah, &mut al);
                if s8(ah) > s8(fp.rlim) && n > 2 {
                    n = 0;
                    ah = ah.wrapping_sub(1);
                    al = al.wrapping_sub(1);
                }
            }
            let (mut ah, mut al, mut n, mut kk) = (fp.rf, fp.rf.wrapping_add(1), 0, 0i8);
            for r in 0..rows {
                put_row(fb, half + r, al, ah);
                n += 1;
                std::mem::swap(&mut ah, &mut al);
                if n > 2 {
                    kk = kk.wrapping_add(1);
                    if kk > 45 && s8(ah) < s8(fp.fl) {
                        n = 0;
                        ah = ah.wrapping_add(1);
                        al = al.wrapping_add(1);
                    }
                }
            }
        }
    }

    fn info(scene: &Scene, x: i32, y: i32) -> u16 {
        if (0..64).contains(&x) && (0..64).contains(&y) {
            scene.info[(y * 64 + x) as usize] & 0xFF
        } else {
            0
        }
    }

    fn tile(scene: &Scene, x: i32, y: i32) -> u8 {
        if (0..64).contains(&x) && (0..64).contains(&y) {
            scene.tiles[(x * 64 + y) as usize]
        } else {
            1
        }
    }

    /// Wall page for a solid face, with the plane-1 125 door-frame rule.
    fn wall_page(scene: &Scene, x: i32, y: i32, fromx: i32, fromy: i32) -> u16 {
        let t = Self::tile(scene, x, y);
        if Self::info(scene, x, y) == 125 {
            let f = Self::tile(scene, fromx, fromy);
            let frame = matches!(f, 13..=16 | 29..=32 | 49..=52 | 65..=68 | 69..=72);
            if f & 0x80 != 0 || frame {
                return DOOR_FRAME_PAGE;
            }
        }
        u16::from(t & 0x7F).wrapping_sub(1)
    }

    /// One ray pass over every column.
    #[allow(clippy::too_many_arguments)]
    fn trace_pass(
        &mut self,
        fb: &mut Framebuffer,
        x0: i32,
        y0: i32,
        scene: &Scene,
        tex: &dyn Textures,
        ray: &Ray,
        pass: Pass,
    ) {
        let viewangle = scene.player_angle.rem_euclid(360);
        let midangle = viewangle * (FINEANGLES / 360);
        let focaltx = ray.viewx >> 16;
        let focalty = ray.viewy >> 16;
        let xpd = (ray.viewx & 0xFFFF) as u16;
        let xpu = i32::from(xpd).wrapping_neg() as u16;
        let ypd = (ray.viewy & 0xFFFF) as u16;
        let ypu = i32::from(ypd).wrapping_neg() as u16;
        if pass == Pass::B {
            self.see_through = 0;
        }
        let mut post = Post { postx: 0, width: 0, page: 0, column: 0, lastside: -1, key: -1 };
        let ft = self.tables.finetangent.clone();
        for pixx in 0..self.w {
            let mut angle = midangle + self.pixelangle[pixx];
            if angle < 0 {
                angle += FINEANGLES;
            }
            if angle >= DEG360 {
                angle -= FINEANGLES;
            }
            let (xs, ys, xstep, ystep, xpartial, ypartial) = if angle < DEG90 {
                (1, -1, ft[(DEG90 - 1 - angle) as usize], -ft[angle as usize], xpu, ypd)
            } else if angle < DEG180 {
                (
                    -1,
                    -1,
                    -ft[(angle - DEG90) as usize],
                    -ft[(DEG180 - 1 - angle) as usize],
                    xpd,
                    ypd,
                )
            } else if angle < DEG270 {
                (-1, 1, -ft[(DEG270 - 1 - angle) as usize], ft[(angle - DEG180) as usize], xpd, ypu)
            } else {
                (1, 1, ft[(angle - DEG270) as usize], ft[(DEG360 - 1 - angle) as usize], xpu, ypu)
            };
            let mut yint = ray.viewy.wrapping_add(partial_mul(ystep, xpartial));
            let mut xtile = focaltx + xs;
            let mut xint = ray.viewx.wrapping_add(partial_mul(xstep, ypartial));
            let mut ytile = focalty + ys;
            let mut vert = true;
            let mut hit = None;
            for _ in 0..256 {
                let (cx, cy, along, step) = if vert {
                    let yt = yint >> 16;
                    if (ys == -1 && yt <= ytile) || (ys == 1 && yt >= ytile) {
                        vert = false;
                        continue;
                    }
                    (xtile, yt, yint, ystep)
                } else {
                    let xt = xint >> 16;
                    if (xs == -1 && xt <= xtile) || (xs == 1 && xt >= xtile) {
                        vert = true;
                        continue;
                    }
                    (xt, ytile, xint, xstep)
                };
                if !(0..64).contains(&cx) || !(0..64).contains(&cy) {
                    break;
                }
                let (fromx, fromy) = if vert { (cx - xs, cy) } else { (cx, cy - ys) };
                match self.cell(scene, pass, cx, cy, along, step, vert, fromx, fromy) {
                    CellResult::Pass => {}
                    CellResult::Stop => break,
                    CellResult::Hit(h) => {
                        hit = Some(h);
                        break;
                    }
                }
                if pass != Pass::C {
                    self.spotvis[(cx * 64 + cy) as usize] = true;
                }
                if vert {
                    xtile += xs;
                    yint = yint.wrapping_add(ystep);
                } else {
                    ytile += ys;
                    xint = xint.wrapping_add(xstep);
                }
            }
            if pass == Pass::A {
                continue;
            }
            match hit {
                Some(h) => self.hit(fb, x0, y0, pixx, h, xs, ys, scene, ray, &mut post, tex),
                None => {
                    if pass == Pass::B {
                        self.wallheight[pixx] = 8;
                    }
                }
            }
        }
        if pass != Pass::A && post.lastside != -1 {
            self.scale_post(fb, x0, y0, &post, tex, scene);
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn cell(
        &mut self,
        scene: &Scene,
        pass: Pass,
        x: i32,
        y: i32,
        along: i32,
        step: i32,
        vertical: bool,
        fromx: i32,
        fromy: i32,
    ) -> CellResult {
        let info = Self::info(scene, x, y);
        let t = Self::tile(scene, x, y);
        let (xi, yi) = if vertical { (x << 16, along) } else { (along, y << 16) };
        if pass == Pass::C {
            if matches!(info, 104 | 105) {
                let page = u16::from(t & 0x7F).wrapping_sub(1);
                return CellResult::Hit(Hit::Wall { vertical, x, y, xi, yi, page });
            }
            if matches!(info, 103 | 106 | 107) {
                let half = along.wrapping_add(step >> 1);
                if half >> 16 != along >> 16 {
                    return CellResult::Pass;
                }
                let (offset, page) = if t & 0x80 != 0 {
                    let d = scene.doors.get(usize::from(t & 0x7F)).copied().unwrap_or_default();
                    if ((half & 0xFFFF) as u16) < d.position {
                        return CellResult::Pass;
                    }
                    (d.position, d.page)
                } else {
                    (0, u16::from(t).wrapping_sub(1))
                };
                let (xi, yi) =
                    if vertical { ((x << 16) | 0x8000, half) } else { (half, (y << 16) | 0x8000) };
                return CellResult::Hit(Hit::Thin { vertical, x, y, xi, yi, page, offset });
            }
            return if self.spotvis[(x * 64 + y) as usize] {
                CellResult::Pass
            } else {
                CellResult::Stop
            };
        }
        if (103..=107).contains(&info) {
            if pass == Pass::B {
                self.see_through += 1;
            }
            return CellResult::Pass;
        }
        if t == 0 {
            return CellResult::Pass;
        }
        if t & 0x80 == 0 {
            let page = Self::wall_page(scene, x, y, fromx, fromy);
            return CellResult::Hit(Hit::Wall { vertical, x, y, xi, yi, page });
        }
        if matches!(info, 98 | 101 | 102) {
            let off = (step as u32).wrapping_mul(u32::from(scene.pwall_pos)) as i32 >> 6;
            let np = along.wrapping_add(off);
            if np >> 16 != along >> 16 {
                return CellResult::Pass;
            }
            let (xi, yi) = if vertical { (x << 16, np) } else { (np, y << 16) };
            let page = u16::from(t & 0x7F).wrapping_sub(1);
            return CellResult::Hit(Hit::PushWall { vertical, x, y, xi, yi, page });
        }
        let d = scene.doors.get(usize::from(t & 0x7F)).copied().unwrap_or_default();
        let half = along.wrapping_add(step >> 1);
        if half >> 16 == along >> 16 && (half & 0xFFFF) as u16 >= d.position {
            let (xi, yi) =
                if vertical { ((x << 16) | 0x8000, half) } else { (half, (y << 16) | 0x8000) };
            return CellResult::Hit(Hit::Thin {
                vertical,
                x,
                y,
                xi,
                yi,
                page: d.page,
                offset: d.position,
            });
        }
        CellResult::Pass
    }

    #[allow(clippy::too_many_arguments)]
    fn hit(
        &mut self,
        fb: &mut Framebuffer,
        x0: i32,
        y0: i32,
        pixx: usize,
        hit: Hit,
        xs: i32,
        ys: i32,
        scene: &Scene,
        ray: &Ray,
        post: &mut Post,
        tex: &dyn Textures,
    ) {
        let (side, key, texture, page, xi, yi) = match hit {
            Hit::Wall { vertical: true, x, xi, yi, page, .. } => {
                let mut texture = ((yi >> 4) & 0xFC0) as u16;
                let mut xi = xi;
                if xs == -1 {
                    texture = 0xFC0 - texture;
                    xi += TILEGLOBAL;
                }
                (1, x * 1024 + i32::from(page), texture, page, xi, yi)
            }
            Hit::Wall { vertical: false, y, xi, yi, page, .. } => {
                let mut texture = ((xi >> 4) & 0xFC0) as u16;
                let mut yi = yi;
                if ys == -1 {
                    yi += TILEGLOBAL;
                } else {
                    texture = 0xFC0 - texture;
                }
                (0, y * 1024 + i32::from(page), texture, page, xi, yi)
            }
            Hit::Thin { vertical, x, y, xi, yi, page, offset } => {
                let along = if vertical { yi } else { xi };
                let texture = ((along.wrapping_sub(i32::from(offset)) >> 4) & 0xFC0) as u16;
                (2, x * 64 + y, texture, page, xi, yi)
            }
            Hit::PushWall { vertical, x, y, xi, yi, page } => {
                let offset = i32::from(scene.pwall_pos) << 10;
                let (mut xi, mut yi) = (xi, yi);
                let texture = if vertical {
                    let t = ((yi >> 4) & 0xFC0) as u16;
                    if xs == -1 {
                        xi += TILEGLOBAL - offset;
                        t
                    } else {
                        xi += offset;
                        0xFC0 - t
                    }
                } else {
                    let t = ((xi >> 4) & 0xFC0) as u16;
                    if ys == -1 {
                        yi += TILEGLOBAL - offset;
                        t
                    } else {
                        yi += offset;
                        0xFC0 - t
                    }
                };
                (3, x * 64 + y, texture, page, xi, yi)
            }
        };
        let height = self.calc_height(xi, yi, ray);
        self.wallheight[pixx] = height;
        if post.lastside == side && post.key == key {
            if texture >> 6 == post.column {
                post.width += 1;
                self.wallheight[pixx] = self.wallheight[pixx - 1];
                return;
            }
            self.scale_post(fb, x0, y0, post, tex, scene);
            post.column = texture >> 6;
            post.width = 1;
            post.postx = pixx;
            return;
        }
        if post.lastside != -1 {
            self.scale_post(fb, x0, y0, post, tex, scene);
        }
        *post = Post { postx: pixx, width: 1, page, column: texture >> 6, lastside: side, key };
    }

    /// `CalcHeight`: `heightnumerator / (nx >> 8)`, at least 8.
    fn calc_height(&self, xi: i32, yi: i32, ray: &Ray) -> i32 {
        let gxt = fixed_by_frac(xi.wrapping_sub(ray.viewx), ray.viewcos);
        let gyt = fixed_by_frac(yi.wrapping_sub(ray.viewy), ray.viewsin);
        let nx = gxt.wrapping_sub(gyt).max(MINDIST);
        let div =
            if self.cfg.is_classic() { i64::from((nx >> 8) as i16) } else { i64::from(nx >> 8) };
        ((self.heightnumerator / div.max(1)) as i32).max(8)
    }

    /// `ScalePost`: one shade for the whole post; 0xFF texels transparent.
    fn scale_post(
        &self,
        fb: &mut Framebuffer,
        x0: i32,
        y0: i32,
        post: &Post,
        tex: &dyn Textures,
        scene: &Scene,
    ) {
        let Some(wall) = tex.wall(post.page) else { return };
        let height = self.wallheight[post.postx];
        let sc = self.scaler((height >> 3).min(self.maxscale));
        let shade = self.shade_for(height, scene);
        let col = &wall[post.column as usize * 64..post.column as usize * 64 + 64];
        let vh = self.h as i32;
        for (src, &(start, end)) in sc.rows.iter().enumerate() {
            let Some(c) = shade_texel(col[src], shade) else { continue };
            for y in start.max(0)..end.min(vh) {
                for x in post.postx..post.postx + post.width {
                    fb.put(x0 + x as i32, y0 + y, c);
                }
            }
        }
    }

    /// Textured ceiling/floor quads for visible tiles (`1b4a:1494`, `1b4a:162b`).
    fn floor_tiles(
        &mut self,
        fb: &mut Framebuffer,
        x0: i32,
        y0: i32,
        scene: &Scene,
        tex: &dyn Textures,
        ray: &Ray,
    ) {
        let ptx = scene.player_x >> 16;
        let pty = scene.player_y >> 16;
        let d = scene.floor.quad;
        let k = self.k();
        let mut corners = [(0i32, 0i32); 4];
        for x in 0..64i32 {
            for y in 0..64i32 {
                if !(self.spotvis[(x * 64 + y) as usize] || (x == ptx && y == pty)) {
                    continue;
                }
                let cx = (x << 16) + 0x8000;
                let cy = (y << 16) + 0x8000;
                let mut h3 = 0;
                for (kk, (dx, dy)) in [(d, d), (d, -d), (-d, -d), (-d, d)].into_iter().enumerate() {
                    let gx = cx + dx - ray.viewx;
                    let gy = cy + dy - ray.viewy;
                    let nx = fixed_by_frac(gx, ray.viewcos)
                        .wrapping_sub(fixed_by_frac(gy, ray.viewsin))
                        .max(MINDIST);
                    let ny =
                        fixed_by_frac(gy, ray.viewcos).wrapping_add(fixed_by_frac(gx, ray.viewsin));
                    let sx = self.centerx + (i64::from(ny) * self.scale / i64::from(nx)) as i32;
                    let h = (self.heightnumerator / i64::from(nx >> 8).max(1)) as i32;
                    corners[kk] = (sx, (self.h as i32) / 2 - (h >> 3));
                    if kk == 3 {
                        h3 = h / k;
                    }
                }
                let shade = scene.shade_table[((h3 >> 2) - 5).clamp(0, 255) as usize];
                let (fp, cp) = scene.floor.sets[Self::floor_code(scene, x, y)];
                let (Some(fpage), Some(cpage)) = (
                    tex.wall(u16::from(fp).wrapping_sub(1)),
                    tex.wall(u16::from(cp).wrapping_sub(1)),
                ) else {
                    continue;
                };
                self.raster_quad(fb, x0, y0, &corners, fpage, cpage, shade, d < 0x8000);
            }
        }
    }

    /// Floor code 0–9 for a tile (docs/re/RENDERER.md §3.2).
    fn floor_code(scene: &Scene, x: i32, y: i32) -> usize {
        let rule = |x: i32, y: i32, byte: bool| -> Option<usize> {
            let (x, y) = (x.rem_euclid(64), y.rem_euclid(64));
            match Self::tile(scene, x, y) {
                0x19 | 0x1D..=0x20 => return Some(3),
                0x31..=0x34 | 0x41..=0x48 | 0x75..=0x78 => return Some(0),
                0x39..=0x3C => return Some(7),
                _ => {}
            }
            let p = scene.plane0[(y * 64 + x) as usize];
            // The cell itself reads plane 0 as a byte, neighbours as a word.
            let c =
                if byte { u16::from((p as u8).wrapping_sub(0x90)) } else { p.wrapping_sub(0x90) };
            (c <= 9).then_some(c as usize)
        };
        if let Some(c) = rule(x, y, true) {
            return c;
        }
        // Neighbour table DS:0x0D5C: (0,-1) missing, (0,+1) twice (quirk).
        for (dx, dy) in [(0, 1), (1, 0), (0, 1), (-1, 0), (-1, -1), (1, -1), (1, 1), (-1, 1)] {
            if let Some(c) = rule(x + dx, y + dy, false) {
                return c;
            }
        }
        9
    }

    /// `30d6:1814`/`30d6:1a21`: column spans, screen-space texturing.
    #[allow(clippy::too_many_arguments)]
    fn raster_quad(
        &self,
        fb: &mut Framebuffer,
        x0: i32,
        y0: i32,
        c: &[(i32, i32); 4],
        fpage: &[u8],
        cpage: &[u8],
        shade: u8,
        grout: bool,
    ) {
        let k = self.k();
        let half = self.h as i32 / 2;
        let maxx = c.iter().map(|p| p.0).max().unwrap_or(0).min(self.w as i32);
        let minx = c.iter().map(|p| p.0).min().unwrap_or(0).max(0);
        let cbase = cpage[0] & 0xF0;
        let fbase = fpage[0] & 0xF0;
        for col in minx..maxx {
            let (mut ymin, mut ymax) = (200 * k, -1);
            for (a, b) in [(3usize, 0usize), (0, 1), (1, 2), (2, 3)] {
                let (x1, y1) = c[a];
                let (x2, y2) = c[b];
                if (x1 < col && x2 < col) || (x1 > col && x2 > col) || x1 == x2 {
                    continue;
                }
                let slope = ((y2 - y1) << 7) / (x2 - x1);
                let y = ((y1 << 7) + (col - x1) * slope) >> 7;
                ymin = ymin.min(y);
                ymax = ymax.max(y);
            }
            ymax = ymax.min(half);
            ymin = ymin.max(0);
            if ymin > ymax {
                continue;
            }
            let count = ymax - ymin;
            let u = (((col - minx) / k) % 64) as usize;
            let mut v = 0usize;
            for i in 0..count {
                let t = if grout && i == count - 1 { 0 } else { v * 64 + u };
                let cpx = cpage[t].saturating_sub(shade).max(cbase);
                let fpx = fpage[t].saturating_sub(shade).max(fbase);
                fb.put(x0 + col, y0 + ymin + i, cpx);
                fb.put(x0 + col, y0 + (self.h as i32 - 1 - ymin - i), fpx);
                if (i + 1) % k == 0 {
                    v += 1;
                    if v == 63 {
                        v = 0;
                    }
                }
            }
        }
    }

    /// `DrawScaleds` list: visible statics and actors, sorted far to near.
    fn visible_sprites(&self, scene: &Scene, ray: &Ray) -> Vec<(i32, i32, u16)> {
        let mut vis = Vec::new();
        for s in scene.sprites {
            if self.cfg.is_classic() && vis.len() >= MAXVISIBLE - 1 {
                break;
            }
            let (tx, ty) = (i32::from(s.tile_x), i32::from(s.tile_y));
            let seen = |dx: i32, dy: i32| {
                let (x, y) = (tx + dx, ty + dy);
                (0..64).contains(&x) && (0..64).contains(&y) && self.spotvis[(x * 64 + y) as usize]
            };
            let open = |dx: i32, dy: i32| Self::tile(scene, tx + dx, ty + dy) == 0;
            let (gx, gy, size) = if s.is_static {
                if !seen(0, 0) {
                    continue;
                }
                ((tx << 16) + 0x8000 - ray.viewx, (ty << 16) + 0x8000 - ray.viewy, 0x2000)
            } else {
                let visible = seen(0, 0)
                    || [(-1, 0), (1, 0), (-1, -1), (0, -1), (1, -1), (-1, 1), (0, 1), (1, 1)]
                        .iter()
                        .any(|&(dx, dy)| seen(dx, dy) && open(dx, dy));
                if !visible {
                    continue;
                }
                (s.x.wrapping_sub(ray.viewx), s.y.wrapping_sub(ray.viewy), ACTORSIZE)
            };
            let nx = fixed_by_frac(gx, ray.viewcos)
                .wrapping_sub(fixed_by_frac(gy, ray.viewsin))
                .wrapping_sub(size);
            let ny = fixed_by_frac(gy, ray.viewcos).wrapping_add(fixed_by_frac(gx, ray.viewsin));
            if nx < MINDIST {
                continue;
            }
            let viewx = self.centerx + (i64::from(ny) * self.scale / i64::from(nx)) as i32;
            let div = if self.cfg.is_classic() {
                i64::from((nx >> 8) as i16)
            } else {
                i64::from(nx >> 8)
            };
            let height = (self.heightnumerator / div.max(1)) as i32;
            if height == 0 {
                continue;
            }
            let shape = if s.rotate != 0 {
                s.shape + self.calc_rotate(s, viewx, scene.player_angle)
            } else {
                s.shape
            };
            vis.push((viewx, height, shape));
        }
        vis.sort_by_key(|v| v.1);
        vis
    }

    /// `CalcRotate`, with the screen offset measured in original pixels.
    fn calc_rotate(&self, s: &SpriteRef, viewx: i32, player_angle: i32) -> u16 {
        let off = (self.centerx - viewx) / self.k();
        let angle = ((player_angle + off / 8 - 180) - s.facing + 22).rem_euclid(360);
        if s.rotate == 2 { (4 * (angle / 180)) as u16 } else { (angle / 45) as u16 }
    }

    fn draw_sprites(
        &self,
        fb: &mut Framebuffer,
        x0: i32,
        y0: i32,
        vis: &[(i32, i32, u16)],
        scene: &Scene,
        tex: &dyn Textures,
    ) {
        for &(viewx, height, shape) in vis {
            self.scale_shape(fb, x0, y0, viewx, shape, height, tex, self.shade_for(height, scene));
        }
    }

    /// `ScaleShape`: multi-column spans test only their end columns.
    #[allow(clippy::too_many_arguments)]
    fn scale_shape(
        &self,
        fb: &mut Framebuffer,
        x0: i32,
        y0: i32,
        xcenter: i32,
        shape: u16,
        height: i32,
        tex: &dyn Textures,
        shade: u8,
    ) {
        let Some(sh) = tex.sprite(shape) else { return };
        let scale = height >> 3;
        if scale == 0 || (self.cfg.is_classic() && scale > self.maxscale) {
            return;
        }
        let sc = self.scaler(scale.min(self.maxscale));
        let vw = self.w as i32;
        let wh = |x: i32| self.wallheight[x as usize];
        let draw = |fb: &mut Framebuffer, x: i32, w: i32, srcx: i32| {
            self.scale_line(fb, x0, y0, x, w, sh, srcx as usize, sc, shade);
        };
        let mut slinex = xcenter;
        let mut srcx = 32i32;
        loop {
            srcx -= 1;
            if srcx < i32::from(sh.left) || slinex <= 0 {
                break;
            }
            let mut w = sc.width[srcx as usize];
            if w == 0 {
                continue;
            }
            if w == 1 {
                slinex -= 1;
                if slinex < vw && wh(slinex) < height {
                    draw(fb, slinex, 1, srcx);
                }
                continue;
            }
            if slinex > vw {
                slinex -= w;
                w = vw - slinex;
                if w < 1 {
                    continue;
                }
            } else {
                w = w.min(slinex);
                slinex -= w;
            }
            let leftvis = wh(slinex) < height;
            let rightvis = wh(slinex + w - 1) < height;
            if leftvis {
                while !rightvis && w > 0 && wh(slinex + w - 1) >= height {
                    w -= 1;
                }
                draw(fb, slinex, w, srcx);
            } else {
                if !rightvis {
                    continue;
                }
                while w > 0 && wh(slinex) >= height {
                    slinex += 1;
                    w -= 1;
                }
                draw(fb, slinex, w, srcx);
                break;
            }
        }
        let mut slinex = xcenter;
        let mut srcx = if sh.left < 31 { 31 } else { i32::from(sh.left) - 1 };
        let mut w = 0i32;
        loop {
            srcx += 1;
            if srcx > i32::from(sh.right) {
                break;
            }
            slinex += w;
            if slinex >= vw {
                break;
            }
            w = sc.width[srcx as usize];
            if w == 0 {
                continue;
            }
            if w == 1 {
                if slinex >= 0 && wh(slinex) < height {
                    draw(fb, slinex, 1, srcx);
                }
                continue;
            }
            if slinex < 0 {
                if w <= -slinex {
                    continue;
                }
                w += slinex;
                slinex = 0;
            } else if slinex + w > vw {
                w = vw - slinex;
            }
            let leftvis = wh(slinex) < height;
            let rightvis = wh(slinex + w - 1) < height;
            if leftvis {
                if rightvis {
                    draw(fb, slinex, w, srcx);
                } else {
                    while w > 0 && wh(slinex + w - 1) >= height {
                        w -= 1;
                    }
                    draw(fb, slinex, w, srcx);
                    break;
                }
            } else if rightvis {
                while w > 0 && wh(slinex) >= height {
                    slinex += 1;
                    w -= 1;
                }
                draw(fb, slinex, w, srcx);
            }
        }
    }

    /// `ScaleLine`: `w` screen columns from source column `srcx`.
    #[allow(clippy::too_many_arguments)]
    fn scale_line(
        &self,
        fb: &mut Framebuffer,
        x0: i32,
        y0: i32,
        x: i32,
        w: i32,
        sh: &Shape,
        srcx: usize,
        sc: &Scaler,
        shade: u8,
    ) {
        let vh = self.h as i32;
        for post in &sh.posts[srcx] {
            for src in post.start as usize..post.end as usize {
                let Some(c) = shade_texel(sh.pixels[srcx * 64 + src], shade) else { continue };
                let (start, end) = sc.rows[src];
                for y in start.max(0)..end.min(vh) {
                    for col in x..x + w {
                        fb.put(x0 + col, y0 + y, c);
                    }
                }
            }
        }
    }

    /// `DrawPlayerWeapon` (1b4a:1013): two sprites at ×1.1, placed by the
    /// per-view-height planar offsets (208c:0009); unclipped, unshaded.
    fn draw_weapon(
        &self,
        fb: &mut Framebuffer,
        x0: i32,
        y0: i32,
        wv: WeaponView,
        tex: &dyn Textures,
    ) {
        let k = self.k();
        let base_w = i32::from(self.cfg.viewsize) * 16;
        let vh0 = i32::from(self.cfg.viewsize) * 8;
        let scale = (f64::from(vh0 / 4) * 1.1) as i32;
        let (off1, off2) = weapon_offsets(vh0);
        // A planar offset moves the image `off / 80` rows and `(off % 80)·4` pixels;
        // decompose so the horizontal part stays within ±160 pixels.
        let decompose = |off: i32| {
            let dy = (off + 40).div_euclid(80);
            (dy, (off - dy * 80) * 4)
        };
        let sc = Scaler::build(scale * 2 * k, vh0 * k);
        let xcenter = (base_w / 2 + wv.bob) * k + (self.w as i32 - base_w * k) / 2;
        for (shape, off) in [(wv.left, off1), (wv.right, off1 + off2)] {
            let Some(sh) = tex.sprite(shape) else { continue };
            let (dy, dx) = decompose(off);
            let (ox, oy) = (x0 + dx * k, y0 + dy * k);
            let mut slinex = xcenter;
            let mut srcx = 32i32;
            loop {
                srcx -= 1;
                if srcx < i32::from(sh.left) {
                    break;
                }
                let w = sc.width[srcx as usize];
                if w == 0 {
                    continue;
                }
                slinex -= w;
                weapon_line(fb, ox, oy, slinex, w, sh, srcx as usize, &sc);
            }
            let mut slinex = xcenter;
            let mut srcx = if sh.left < 31 { 31 } else { i32::from(sh.left) - 1 };
            loop {
                srcx += 1;
                if srcx > i32::from(sh.right) {
                    break;
                }
                let w = sc.width[srcx as usize];
                if w == 0 {
                    continue;
                }
                weapon_line(fb, ox, oy, slinex, w, sh, srcx as usize, &sc);
                slinex += w;
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn weapon_line(
    fb: &mut Framebuffer,
    x0: i32,
    y0: i32,
    x: i32,
    w: i32,
    sh: &Shape,
    srcx: usize,
    sc: &Scaler,
) {
    for post in &sh.posts[srcx] {
        for src in post.start as usize..post.end as usize {
            let Some(c) = shade_texel(sh.pixels[srcx * 64 + src], 0) else { continue };
            let (start, end) = sc.rows[src];
            for y in start..end {
                for col in x..x + w {
                    fb.put(x0 + col, y0 + y, c);
                }
            }
        }
    }
}

/// Planar offsets of the two weapon halves by view height (ghidra 208c:0009).
fn weapon_offsets(viewheight: i32) -> (i32, i32) {
    match viewheight {
        88 => (0x6E0, 10),
        96 => (0x6D6, 12),
        104 => (0x820, 12),
        112 => (0x816, 14),
        120 => (0x960, 14),
        128 => (0x906, 16),
        136 => (0xA8C, 12),
        144 => (0xA46, 18),
        152 => (0xE0C, 14),
        160 => (0xB36, 21),
        200 => (0xF96, 24),
        _ => (0, 0),
    }
}

/// `xpartialbyystep`: signed step × unsigned 16-bit fraction, magnitude truncated.
fn partial_mul(step: i32, partial: u16) -> i32 {
    let r = ((u64::from(step.unsigned_abs()) * u64::from(partial)) >> 16) as u32 as i32;
    if step < 0 { r.wrapping_neg() } else { r }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct NoTex;
    impl Textures for NoTex {
        fn wall(&self, _: u16) -> Option<&[u8]> {
            None
        }
        fn sprite(&self, _: u16) -> Option<&Shape> {
            None
        }
    }

    #[test]
    fn projection_matches_original_constants() {
        // View size 16: 256x128, scale = trunc(128 * 44800 / 32768) = 175.
        let v = View3D::new(ViewConfig::classic(16));
        assert_eq!(v.scale, 175);
        assert_eq!(v.heightnumerator, 175 << 10);
        assert_eq!(v.centerx, 127);
    }

    #[test]
    fn shade_table_matches_report() {
        let t = build_shade_table(false);
        assert_eq!((t[0], t[3], t[4], t[72], t[161]), (16, 13, 11, 1, 0));
        let d = build_shade_table(true);
        assert_eq!((d[5], d[157]), (15, 1));
    }

    #[test]
    fn texel_shading_rules() {
        assert_eq!(shade_texel(255, 3), None);
        assert_eq!(shade_texel(5, 10), Some(5));
        assert_eq!(shade_texel(40, 3), Some(37));
        assert_eq!(shade_texel(32, 0), Some(0));
        assert_eq!(shade_texel(36, 8), Some(0));
    }

    #[test]
    fn renders_a_room_without_panicking() {
        let mut tiles = vec![0u8; 4096];
        for i in 0..64 {
            for (x, y) in [(i, 0), (i, 63), (0, i), (63, i)] {
                tiles[x * 64 + y] = 5;
            }
        }
        let info = vec![18u16; 4096];
        let plane0 = vec![144u16; 4096];
        let table = build_shade_table(false);
        for cfg in [ViewConfig::classic(18), ViewConfig { viewsize: 18, k: 3, width: 1200 }] {
            let mut v = View3D::new(cfg);
            let mut fb = Framebuffer::new(cfg.width, cfg.height());
            for angle in (0..360).step_by(13) {
                let scene = Scene {
                    tiles: &tiles,
                    info: &info,
                    plane0: &plane0,
                    doors: &[],
                    pwall_pos: 0,
                    floor: FloorParams {
                        mode: 2,
                        cc: 81,
                        fc: 116,
                        rc: 46,
                        rlim: 33,
                        rf: 65,
                        fl: 78,
                        quad: 0x7600,
                        sets: [(81, 82); 10],
                    },
                    shade_table: &table,
                    shade_bias: 0,
                    player_x: 32 << 16,
                    player_y: 32 << 16,
                    player_angle: angle,
                    sprites: &[],
                    weapon: Some(WeaponView { left: 544, right: 584, bob: 0 }),
                };
                v.render(&mut fb, 0, 0, &scene, &NoTex);
            }
            assert!(v.spotvis.iter().filter(|&&s| s).count() > 100);
        }
    }
}
