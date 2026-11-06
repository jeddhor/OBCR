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
use crate::xbrz::{HiTexture, Mix};

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
    /// Enhanced interpolation: the view angle in `FINEANGLES` units (0..3600).
    /// `None` (Classic) uses `player_angle` and the original tables exactly.
    pub player_fine_angle: Option<i32>,
    pub sprites: &'a [SpriteRef],
    pub weapon: Option<WeaponView>,
    /// Enhanced SMOOTH FLOORS (ADR-019): perspective-correct, gapless floors
    /// and ceilings with smooth distance shading. Ignored in Classic.
    pub smooth_floors: bool,
}

/// Source of wall and sprite pixels.
pub trait Textures {
    /// 4096 column-major palette indices.
    fn wall(&self, page: u16) -> Option<&[u8]>;
    fn sprite(&self, index: u16) -> Option<&Shape>;
    /// xBRZ factor of the upscaled textures below (Enhanced, ADR-020), or 1
    /// when there are none.
    fn hires(&self) -> usize {
        1
    }
    /// Upscaled wall page (repeats sideways).
    fn wall_hi(&self, _page: u16) -> Option<&HiTexture> {
        None
    }
    /// Upscaled floor or ceiling page (repeats both ways).
    fn flat_hi(&self, _page: u16) -> Option<&HiTexture> {
        None
    }
    /// Upscaled sprite.
    fn sprite_hi(&self, _index: u16) -> Option<&HiTexture> {
        None
    }
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
    /// Texture column: 0–63, or 0–64n−1 with upscaled textures.
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
    /// xBRZ factor of the textures this frame (1 = original texels).
    hires: usize,
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
            hires: 1,
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
        let classic = self.cfg.is_classic();
        self.hires = if classic { 1 } else { tex.hires().clamp(1, 6) };
        let smooth = scene.smooth_floors && !classic && scene.floor.mode != 0;
        // Smooth floors cover every pixel the walls leave.
        if !smooth {
            self.background(fb, x0, y0, &scene.floor);
        }
        self.spotvis.fill(false);
        self.trace_pass(fb, x0, y0, scene, tex, &ray, Pass::A);
        if scene.floor.mode != 0 && !smooth {
            self.floor_tiles(fb, x0, y0, scene, tex, &ray);
        }
        self.trace_pass(fb, x0, y0, scene, tex, &ray, Pass::B);
        if smooth {
            self.floor_smooth(fb, x0, y0, scene, tex, &ray);
        }
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
        let (viewsin, viewcos) = match scene.player_fine_angle {
            Some(f) if f.rem_euclid(FINEANGLES / 360) != 0 => {
                let r = f64::from(f) * std::f64::consts::TAU / f64::from(FINEANGLES);
                (sign_magnitude(r.sin()), sign_magnitude(r.cos()))
            }
            _ => {
                let viewangle = scene.player_angle.rem_euclid(360) as usize;
                (self.tables.sin(viewangle), self.tables.cos(viewangle))
            }
        };
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
        let midangle = match scene.player_fine_angle {
            Some(f) => f.rem_euclid(FINEANGLES),
            None => scene.player_angle.rem_euclid(360) * (FINEANGLES / 360),
        };
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
        // `along`: the intercept's position on the face (its low 16 bits);
        // `flip`: the texture runs the other way (`0xFC0 - texture`).
        let (side, key, along, flip, page, xi, yi) = match hit {
            Hit::Wall { vertical: true, x, xi, yi, page, .. } => {
                let mut xi = xi;
                if xs == -1 {
                    xi += TILEGLOBAL;
                }
                (1, x * 1024 + i32::from(page), yi, xs == -1, page, xi, yi)
            }
            Hit::Wall { vertical: false, y, xi, yi, page, .. } => {
                let mut yi = yi;
                if ys == -1 {
                    yi += TILEGLOBAL;
                }
                (0, y * 1024 + i32::from(page), xi, ys != -1, page, xi, yi)
            }
            Hit::Thin { vertical, x, y, xi, yi, page, offset } => {
                let along = if vertical { yi } else { xi };
                (2, x * 64 + y, along.wrapping_sub(i32::from(offset)), false, page, xi, yi)
            }
            Hit::PushWall { vertical, x, y, xi, yi, page } => {
                let offset = i32::from(scene.pwall_pos) << 10;
                let (mut xi, mut yi) = (xi, yi);
                let (along, flip) = if vertical {
                    if xs == -1 {
                        xi += TILEGLOBAL - offset;
                    } else {
                        xi += offset;
                    }
                    (yi, xs != -1)
                } else {
                    if ys == -1 {
                        yi += TILEGLOBAL - offset;
                    } else {
                        yi += offset;
                    }
                    (xi, ys != -1)
                };
                (3, x * 64 + y, along, flip, page, xi, yi)
            }
        };
        // `texture >> 6` in the original: (along >> 10) & 63, mirrored by flip.
        let column = {
            let span = 64 * self.hires as u32;
            let c = (u32::from(along as u16) * span) >> 16;
            (if flip { span - 1 - c } else { c }) as u16
        };
        let height = self.calc_height(xi, yi, ray);
        self.wallheight[pixx] = height;
        if post.lastside == side && post.key == key {
            if column == post.column {
                post.width += 1;
                self.wallheight[pixx] = self.wallheight[pixx - 1];
                return;
            }
            self.scale_post(fb, x0, y0, post, tex, scene);
            post.column = column;
            post.width = 1;
            post.postx = pixx;
            return;
        }
        if post.lastside != -1 {
            self.scale_post(fb, x0, y0, post, tex, scene);
        }
        *post = Post { postx: pixx, width: 1, page, column, lastside: side, key };
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
        let height = self.wallheight[post.postx];
        let sc = self.scaler((height >> 3).min(self.maxscale));
        let shade = self.shade_for(height, scene);
        let vh = self.h as i32;
        if self.hires > 1
            && let Some(hi) = tex.wall_hi(post.page)
        {
            let cols = post.postx as i32..(post.postx + post.width) as i32;
            let fine = usize::from(post.column).min(hi.size() - 1);
            draw_hi_column(fb, x0, y0, cols, fine, hi, sc, vh, shade);
            return;
        }
        let Some(wall) = tex.wall(post.page) else { return };
        let column = usize::from(post.column) / self.hires;
        let col = &wall[column * 64..column * 64 + 64];
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
                let (fp, cp) = (u16::from(fp).wrapping_sub(1), u16::from(cp).wrapping_sub(1));
                let (Some(fpage), Some(cpage)) = (tex.wall(fp), tex.wall(cp)) else {
                    continue;
                };
                let hi = match (self.hires > 1, tex.flat_hi(fp), tex.flat_hi(cp)) {
                    (true, Some(f), Some(c)) => Some((f, c)),
                    _ => None,
                };
                let pages = QuadPages { floor: fpage, ceiling: cpage, hi };
                self.raster_quad(fb, x0, y0, &corners, &pages, shade, d < 0x8000);
            }
        }
    }

    /// Enhanced SMOOTH FLOORS (ADR-019), after the wall pass: every screen
    /// pixel below a column's wall (and its mirror above) is cast to the map
    /// and textured there, so the texture is in perspective, aligned to the
    /// tile grid and continuous from tile to tile, with no gaps between
    /// tiles. The shade follows the distance row by row through a continuous
    /// version of the floor shade (`shade_table[(h3 >> 2) - 5]`), blending
    /// the two nearest shade steps. Floor sets, codes and shading rules are
    /// the original's.
    fn floor_smooth(
        &self,
        fb: &mut Framebuffer,
        x0: i32,
        y0: i32,
        scene: &Scene,
        tex: &dyn Textures,
        ray: &Ray,
    ) {
        let vh = self.h as i32;
        let half = vh / 2;
        let flat = |p: u8| -> Option<Flat> {
            let page = u16::from(p).wrapping_sub(1);
            let px = tex.wall(page)?;
            let hi = if self.hires > 1 { tex.flat_hi(page) } else { None };
            Some(Flat { page: px, hi, base: px[0] & 0xF0 })
        };
        let sets: [Option<(Flat, Flat)>; 10] = std::array::from_fn(|c| {
            let (fp, cp) = scene.floor.sets[c];
            Some((flat(fp)?, flat(cp)?))
        });
        let mut codes = [0u8; 4096];
        for (i, code) in codes.iter_mut().enumerate() {
            *code = Self::floor_code(scene, (i / 64) as i32, (i % 64) as i32) as u8;
        }

        let curve = smooth_shades(scene.shade_table);
        let k = f64::from(self.k());
        let hn = self.heightnumerator as f64;
        // Per row below the horizon: forward distance and shade.
        let rows: Vec<(f64, u8, u8)> = (0..(vh - half))
            .map(|r| {
                let off = f64::from(r) + 0.5;
                // A wall at distance nx is heightnumerator·256/nx eighth
                // pixels tall, its foot height/8 pixels below the centre.
                let nx = hn * 32.0 / off;
                // h3 = height / k with height = 8·off.
                let s = sample_curve(&curve, 2.0 * off / k - 5.0);
                let si = s.floor();
                (nx, si as u8, ((s - si) * 255.0).round() as u8)
            })
            .collect();
        // First floor row of each column: just below its wall.
        let first: Vec<i32> = self.wallheight[..self.w]
            .iter()
            .map(|&h| {
                let wall = 2 * (h >> 3).min(self.maxscale);
                ((vh - wall) / 2 + wall).max(half)
            })
            .collect();
        let top = first.iter().copied().min().unwrap_or(vh);
        let (sin, cos) = (sign_magnitude_f(ray.viewsin), sign_magnitude_f(ray.viewcos));
        let (vx, vy) = (f64::from(ray.viewx) / 65536.0, f64::from(ray.viewy) / 65536.0);
        let scale = self.scale as f64;
        let l0 = -f64::from(self.centerx) / scale;
        let width = fb.width;
        if y0 < 0 || x0 < 0 || (y0 + vh) as usize > fb.height || x0 as usize + self.w > width {
            return;
        }
        // Distant walls are drawn at least 8 tall (`CalcHeight`), so the rows
        // just below one can reach past it, off the map: those take the
        // nearest point on the map.
        const EDGE: f64 = 64.0 - 1.0 / 65536.0;
        // One floor row (view row y) and its mirrored ceiling row. The point
        // under column x is the view point + nx·(cos + l·sin, l·cos − sin),
        // l = (x − centerx)/scale (nx = gx·cos − gy·sin, ny = gy·cos + gx·sin
        // = l·nx, inverted): it steps linearly along the row.
        let draw_row = |y: i32,
                        floor_row: (&mut [u8], &mut [[u8; 2]]),
                        ceiling_row: (&mut [u8], &mut [[u8; 2]])| {
            if y < top {
                return;
            }
            let (nx, s, frac) = rows[(y - half) as usize];
            let n = nx / 65536.0;
            let (dx, dy) = (n * sin / scale, n * cos / scale);
            let (mut px, mut py) = (vx + n * (cos + l0 * sin), vy + n * (l0 * cos - sin));
            let (fp, fm) = floor_row;
            let (cp, cm) = ceiling_row;
            let (fy, cy) = (y0 + y, y0 + vh - 1 - y);
            for (x, &f) in first.iter().enumerate() {
                if y >= f {
                    let (qx, qy) = (px.clamp(0.0, EDGE), py.clamp(0.0, EDGE));
                    let (tx, ty) = (qx as usize, qy as usize);
                    if let Some((floor, ceiling)) = &sets[usize::from(codes[tx * 64 + ty])] {
                        let (u, v) = (qx - tx as f64, qy - ty as f64);
                        let sx = x0 + x as i32;
                        let i = sx as usize;
                        let d = |row: i32| BAYER[((row & 3) * 4 + (sx & 3)) as usize];
                        (fp[i], fm[i]) = floor.shade(u, v, s, frac, d(fy));
                        (cp[i], cm[i]) = ceiling.shade(u, v, s, frac, d(cy));
                    }
                }
                px += dx;
                py += dy;
            }
        };
        // Floor rows lie in the lower half of the view and their ceiling rows
        // in the upper half, so stripes of rows can be drawn in parallel.
        let (pixels, mix) = fb.planes_mut();
        let (vtop, vmid, vend) = (y0 as usize, (y0 + half) as usize, (y0 + vh) as usize);
        let (up_px, low_px) =
            pixels[vtop * width..vend * width].split_at_mut((vmid - vtop) * width);
        let (up_mx, low_mx) = mix[vtop * width..vend * width].split_at_mut((vmid - vtop) * width);
        let floor_rows = (vh - half) as usize;
        let threads = std::thread::available_parallelism().map_or(4, |n| n.get()).clamp(1, 8);
        let per = floor_rows.div_ceil(threads).max(1);
        let chunk = per * width;
        let draw_row = &draw_row;
        std::thread::scope(|scope| {
            let stripes = low_px
                .chunks_mut(chunk)
                .zip(low_mx.chunks_mut(chunk))
                .zip(up_px.rchunks_mut(chunk).zip(up_mx.rchunks_mut(chunk)));
            for (t, ((fp, fm), (cp, cm))) in stripes.enumerate() {
                scope.spawn(move || {
                    let n = fp.len() / width;
                    let fp_rows = fp.chunks_mut(width).zip(fm.chunks_mut(width));
                    // Ceiling rows run the other way: floor row r of the
                    // stripe mirrors ceiling row n − 1 − r.
                    let cp_rows = cp.chunks_mut(width).zip(cm.chunks_mut(width)).rev();
                    for (r, (f, c)) in fp_rows.zip(cp_rows).enumerate().take(n) {
                        draw_row(half + (t * per + r) as i32, f, c);
                    }
                });
            }
        });
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

    /// `30d6:1814`/`30d6:1a21`: column spans, screen-space texturing. With
    /// upscaled textures each original texel's k×k pixels show its n×n.
    #[allow(clippy::too_many_arguments)]
    fn raster_quad(
        &self,
        fb: &mut Framebuffer,
        x0: i32,
        y0: i32,
        c: &[(i32, i32); 4],
        pages: &QuadPages,
        shade: u8,
        grout: bool,
    ) {
        let (fpage, cpage) = (pages.floor, pages.ceiling);
        let k = self.k();
        let half = self.h as i32 / 2;
        let maxx = c.iter().map(|p| p.0).max().unwrap_or(0).min(self.w as i32);
        let minx = c.iter().map(|p| p.0).min().unwrap_or(0).max(0);
        let cbase = cpage[0] & 0xF0;
        let fbase = fpage[0] & 0xF0;
        if let Some((fhi, chi)) = pages.hi {
            let n = fhi.n;
            let rule = |t: Mix, base: u8| {
                let s = |c: u8| c.saturating_sub(shade).max(base);
                (s(t.a), s(t.b), t.w)
            };
            for col in minx..maxx {
                let Some((ymin, ymax)) = quad_span(c, col, half, k) else { continue };
                let count = ymax - ymin;
                let u = (((col - minx) / k) % 64) as usize * n
                    + ((col - minx) % k) as usize * n / k as usize;
                for i in 0..count {
                    let (fc, fr) = if grout && i == count - 1 {
                        (0, 0)
                    } else {
                        (u, ((i / k) % 63) as usize * n + (i % k) as usize * n / k as usize)
                    };
                    let (a, b, w) = rule(chi.at(fc, fr), cbase);
                    fb.put_mix(x0 + col, y0 + ymin + i, a, b, w);
                    let (a, b, w) = rule(fhi.at(fc, fr), fbase);
                    fb.put_mix(x0 + col, y0 + (self.h as i32 - 1 - ymin - i), a, b, w);
                }
            }
            return;
        }
        for col in minx..maxx {
            let Some((ymin, ymax)) = quad_span(c, col, half, k) else { continue };
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
        if self.hires > 1
            && let Some(hi) = tex.sprite_hi(shape)
        {
            let place = HiPlace { x0, y0, xcenter, cols: 0..vw, rows: self.h as i32, shade };
            draw_hi_shape(fb, &place, sh, hi, sc, Some((&self.wallheight, height)));
            return;
        }
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
            match tex.sprite_hi(shape).filter(|_| self.hires > 1) {
                Some(hi) => {
                    let place = HiPlace::unclipped(ox, oy, xcenter);
                    draw_hi_shape(fb, &place, sh, hi, &sc, None);
                }
                None => simple_scale_shape(fb, ox, oy, xcenter, sh, &sc),
            }
        }
    }

    /// Screen splats (`DrawScreenSplats` 30d6:10d5, RENDERER.md §6.2), drawn
    /// with `SimpleScaleShape` (no ×1.1) centred on the view.
    pub fn draw_splats(
        &self,
        fb: &mut Framebuffer,
        x0: i32,
        y0: i32,
        splats: &[SplatView],
        tex: &dyn Textures,
    ) {
        let k = self.k();
        let base_w = i32::from(self.cfg.viewsize) * 16;
        let vh0 = i32::from(self.cfg.viewsize) * 8;
        for s in splats {
            let Some(sh) = tex.sprite(s.shape) else { continue };
            if s.height <= 0 {
                continue;
            }
            let sc = Scaler::build(s.height * k, vh0 * k);
            let xcenter = s.x * k + (self.w as i32 - base_w * k) / 2;
            match tex.sprite_hi(s.shape).filter(|_| self.hires > 1) {
                Some(hi) => {
                    draw_hi_shape(fb, &HiPlace::unclipped(x0, y0, xcenter), sh, hi, &sc, None)
                }
                None => simple_scale_shape(fb, x0, y0, xcenter, sh, &sc),
            }
        }
    }
}

/// The ceiling rows a floor quad covers in screen column `col`: the edges'
/// intersections (7-bit fixed slopes), clipped to the upper half.
fn quad_span(c: &[(i32, i32); 4], col: i32, half: i32, k: i32) -> Option<(i32, i32)> {
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
    let (ymin, ymax) = (ymin.max(0), ymax.min(half));
    (ymin <= ymax).then_some((ymin, ymax))
}

/// One floor or ceiling texture for [`View3D::floor_smooth`].
struct Flat<'t> {
    page: &'t [u8],
    hi: Option<&'t HiTexture>,
    /// The page's ramp base (`page[0] & 0xF0`): shading never goes below it.
    base: u8,
}

/// 4×4 ordered-dither thresholds (0–255).
const BAYER: [u8; 16] = [8, 136, 40, 168, 200, 72, 232, 104, 56, 184, 24, 152, 248, 120, 216, 88];

impl Flat<'_> {
    /// The pixel for the texel at (`u`, `v`) in 0..1 with shade `s` blended
    /// `frac`/255 toward `s + 1`, as (index, [second index, weight]). A
    /// plain texel spends the blend on the shade; an xBRZ edge texel already
    /// blends two colours, so its shade is dithered against `dither`.
    #[inline]
    fn shade(&self, u: f64, v: f64, s: u8, frac: u8, dither: u8) -> (u8, [u8; 2]) {
        let rule = |c: u8, s: u8| c.saturating_sub(s).max(self.base);
        let s2 = s.saturating_add(1);
        match self.hi {
            Some(hi) => {
                let size = hi.size();
                let col = ((u * size as f64) as usize).min(size - 1);
                let row = ((v * size as f64) as usize).min(size - 1);
                let t = hi.at(col, row);
                if t.w == 0 {
                    (rule(t.a, s), [rule(t.a, s2), frac])
                } else {
                    let d = if frac > dither { s2 } else { s };
                    (rule(t.a, d), [rule(t.b, d), t.w])
                }
            }
            None => {
                let col = ((u * 64.0) as usize).min(63);
                let row = ((v * 64.0) as usize).min(63);
                let t = self.page[col * 64 + row];
                (rule(t, s), [rule(t, s2), frac])
            }
        }
    }
}

/// A continuous version of a stepped shade table, so the shade can change
/// smoothly with distance: linear between the table's steps, with each step
/// at the midpoint of the two levels it separates. `floor` of the result
/// equals the table in the middle of each step; the first and last levels are
/// held flat.
fn smooth_shades(t: &[u8; 256]) -> [f32; 256] {
    let steps: Vec<usize> = (1..256).filter(|&i| t[i] != t[i - 1]).collect();
    let (Some(&first), Some(&last)) = (steps.first(), steps.last()) else {
        return t.map(f32::from);
    };
    let mut pts: Vec<(f32, f32)> = vec![(0.0, f32::from(t[0]))];
    if first > 1 {
        pts.push(((first - 1) as f32, f32::from(t[0])));
    }
    for &i in &steps {
        pts.push((i as f32, (f32::from(t[i - 1]) + f32::from(t[i])) / 2.0));
    }
    // The last step continues at its slope down to the last level.
    let prev = if steps.len() > 1 { steps[steps.len() - 2] } else { 0 };
    let tail = ((last - prev) as f32 / 2.0).max(1.0);
    pts.push(((last as f32 + tail).min(255.0), f32::from(t[255])));
    pts.push((255.0, f32::from(t[255])));
    let mut out = [0f32; 256];
    for (i, o) in out.iter_mut().enumerate() {
        let x = i as f32;
        let k = pts.partition_point(|p| p.0 <= x).clamp(1, pts.len() - 1);
        let ((x0, y0), (x1, y1)) = (pts[k - 1], pts[k]);
        *o = if x1 > x0 { y0 + (y1 - y0) * ((x - x0) / (x1 - x0)).clamp(0.0, 1.0) } else { y1 };
    }
    out
}

/// [`smooth_shades`] at a fractional index (clamped to the table).
fn sample_curve(curve: &[f32; 256], idx: f64) -> f64 {
    let idx = idx.clamp(0.0, 255.0);
    let i = idx.floor() as usize;
    let j = (i + 1).min(255);
    let f = idx - idx.floor();
    f64::from(curve[i]) * (1.0 - f) + f64::from(curve[j]) * f
}

/// A sign-magnitude table value (bit 31 sign, 16-bit fraction) as a float.
fn sign_magnitude_f(v: i32) -> f64 {
    let m = f64::from(v as u32 & 0x7FFF_FFFF) / 65536.0;
    if v < 0 { -m } else { m }
}

/// Floor and ceiling pages of one quad, with their upscaled versions.
struct QuadPages<'t> {
    floor: &'t [u8],
    ceiling: &'t [u8],
    /// (floor, ceiling).
    hi: Option<(&'t HiTexture, &'t HiTexture)>,
}

/// Shades an upscaled texel with the wall and sprite rule
/// ([`shade_texel`]); `None` if transparent.
#[inline]
fn shade_mix(t: Mix, shade: u8) -> Option<(u8, u8, u8)> {
    let a = shade_texel(t.a, shade)?;
    if t.w == 0 {
        return Some((a, a, 0));
    }
    Some((a, shade_texel(t.b, shade).unwrap_or(a), t.w))
}

/// One fine texture column over screen columns `cols`: each source texel's
/// rows (from `sc`) show its n fine rows. Rows at or past `rows` are skipped.
#[allow(clippy::too_many_arguments)]
fn draw_hi_column(
    fb: &mut Framebuffer,
    x0: i32,
    y0: i32,
    cols: std::ops::Range<i32>,
    fine: usize,
    hi: &HiTexture,
    sc: &Scaler,
    rows: i32,
    shade: u8,
) {
    let (top, bottom) = (sc.rows[0].0, sc.rows[63].1);
    let span = i64::from(bottom - top);
    let (fw, fh) = (fb.width as i32, fb.height as i32);
    let xs = (x0 + cols.start).max(0)..(x0 + cols.end).min(fw);
    if span <= 0 || xs.is_empty() {
        return;
    }
    let ys = top.max(0).max(-y0)..bottom.min(rows).min(fh - y0);
    let size = hi.size();
    let col = &hi.texels[fine * size..(fine + 1) * size];
    // Fine rows per screen row, 16.16 fixed point.
    let step = ((size as i64) << 16) / span;
    let width = fb.width;
    let (pixels, mix) = fb.planes_mut();
    for y in ys {
        let row = ((i64::from(y - top) * step) >> 16) as usize;
        let Some((a, b, w)) = shade_mix(col[row.min(size - 1)], shade) else { continue };
        let base = (y0 + y) as usize * width;
        for x in xs.clone() {
            pixels[base + x as usize] = a;
            mix[base + x as usize] = [b, w];
        }
    }
}

/// Where an upscaled shape goes: origin, centre column, the screen columns
/// and rows it may cover, and its shade.
struct HiPlace {
    x0: i32,
    y0: i32,
    xcenter: i32,
    cols: std::ops::Range<i32>,
    rows: i32,
    shade: u8,
}

impl HiPlace {
    /// `SimpleScaleShape` placement: unclipped (the framebuffer clips), shade 0.
    fn unclipped(x0: i32, y0: i32, xcenter: i32) -> Self {
        Self { x0, y0, xcenter, cols: i32::MIN / 2..i32::MAX / 2, rows: i32::MAX / 2, shade: 0 }
    }
}

/// Upscaled `ScaleShape` / `SimpleScaleShape`. Source columns are laid out
/// from `xcenter` with the original widths (including Wolf3D's start at
/// `xcenter` for shapes whose `leftpix` is 31 or more) and each column's
/// span shows its n fine columns. xBRZ can round a shape one texel past
/// `leftpix`/`rightpix`, so one more column is drawn on each side. With
/// `clip`, screen columns whose wall is at least `height` are hidden, one by
/// one rather than by the end-column test of the original.
fn draw_hi_shape(
    fb: &mut Framebuffer,
    at: &HiPlace,
    sh: &Shape,
    hi: &HiTexture,
    sc: &Scaler,
    clip: Option<(&[i32], i32)>,
) {
    let left = usize::from(sh.left).min(63);
    let right_start = if left < 31 { 32 } else { left };
    let mut spans = [(0i32, 0i32); 64];
    let mut x = at.xcenter;
    for (src, span) in spans.iter_mut().enumerate().skip(right_start) {
        *span = (x, sc.width[src]);
        x += sc.width[src];
    }
    let mut x = at.xcenter;
    for src in (0..right_start.min(32)).rev() {
        x -= sc.width[src];
        spans[src] = (x, sc.width[src]);
    }
    let first = if left < 31 { left.saturating_sub(1) } else { left };
    let last = (usize::from(sh.right) + 1).min(63);
    let n = hi.n as i32;
    for (src, &(sx, sw)) in spans.iter().enumerate().take(last + 1).skip(first) {
        if sw <= 0 {
            continue;
        }
        for x in sx.max(at.cols.start)..(sx + sw).min(at.cols.end) {
            if let Some((wall, height)) = clip
                && wall[x as usize] >= height
            {
                continue;
            }
            let fine = src * hi.n + ((x - sx) * n / sw) as usize;
            draw_hi_column(fb, at.x0, at.y0, x..x + 1, fine, hi, sc, at.rows, at.shade);
        }
    }
}

/// One splat to draw: sprite, view-relative centre x and `SimpleScaleShape`
/// height, both in original pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SplatView {
    pub shape: u16,
    pub x: i32,
    pub height: i32,
}

/// `SimpleScaleShape`: columns 31→left and 32→right from `xcenter`, shade 0.
fn simple_scale_shape(
    fb: &mut Framebuffer,
    ox: i32,
    oy: i32,
    xcenter: i32,
    sh: &Shape,
    sc: &Scaler,
) {
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
        weapon_line(fb, ox, oy, slinex, w, sh, srcx as usize, sc);
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
        weapon_line(fb, ox, oy, slinex, w, sh, srcx as usize, sc);
        slinex += w;
    }
}

/// A sine or cosine as the tables store it: 16-bit magnitude, sign in bit 31.
fn sign_magnitude(v: f64) -> i32 {
    let m = ((v.abs() * 65536.0).round() as i32).min(0xFFFF);
    if v < 0.0 && m != 0 { (m as u32 | 0x8000_0000) as i32 } else { m }
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
                    player_fine_angle: None,
                    sprites: &[],
                    weapon: Some(WeaponView { left: 544, right: 584, bob: 0 }),
                    smooth_floors: false,
                };
                v.render(&mut fb, 0, 0, &scene, &NoTex);
            }
            assert!(v.spotvis.iter().filter(|&&s| s).count() > 100);
        }
    }

    /// Walls with diagonal stripes (xBRZ has edges to round), a floor page in
    /// ramp 96–111 and a ceiling page in ramp 112–127 at pages 80 and 81.
    struct StripeTex {
        walls: Vec<Vec<u8>>,
        hi_walls: Vec<HiTexture>,
        hi_flats: Vec<HiTexture>,
        n: usize,
    }

    impl StripeTex {
        fn new(n: usize) -> Self {
            let mut walls = vec![vec![0u8; 4096]; 90];
            for (p, page) in walls.iter_mut().enumerate() {
                for c in 0..64 {
                    for r in 0..64 {
                        page[c * 64 + r] = match p {
                            80 => 100 + 4 * (((c + r) / 16) % 2) as u8,
                            81 => 118 + 4 * (((c + 2 * r) / 16) % 2) as u8,
                            _ => 40 + 6 * (((c + r) / 16) % 2) as u8,
                        };
                    }
                }
            }
            let mut pal = [[0u8; 3]; 256];
            for (i, c) in pal.iter_mut().enumerate() {
                *c = [i as u8, (i * 3) as u8, (i * 7) as u8];
            }
            let m = crate::xbrz::Metric::new(&pal, Some(0xFF));
            let (hi_walls, hi_flats) = if n > 1 {
                (
                    walls.iter().map(|p| HiTexture::from_page(p, n, &m, true, false)).collect(),
                    walls.iter().map(|p| HiTexture::from_page(p, n, &m, true, true)).collect(),
                )
            } else {
                (Vec::new(), Vec::new())
            };
            Self { walls, hi_walls, hi_flats, n }
        }
    }

    impl Textures for StripeTex {
        fn wall(&self, page: u16) -> Option<&[u8]> {
            self.walls.get(usize::from(page)).map(Vec::as_slice)
        }
        fn sprite(&self, _: u16) -> Option<&Shape> {
            None
        }
        fn hires(&self) -> usize {
            self.n
        }
        fn wall_hi(&self, page: u16) -> Option<&HiTexture> {
            self.hi_walls.get(usize::from(page))
        }
        fn flat_hi(&self, page: u16) -> Option<&HiTexture> {
            self.hi_flats.get(usize::from(page))
        }
    }

    /// A 64×64 room walled with tile 5, floor codes all 0.
    fn room() -> (Vec<u8>, Vec<u16>, Vec<u16>) {
        let mut tiles = vec![0u8; 4096];
        for i in 0..64 {
            for (x, y) in [(i, 0), (i, 63), (0, i), (63, i)] {
                tiles[x * 64 + y] = 5;
            }
        }
        (tiles, vec![18u16; 4096], vec![0x90u16; 4096])
    }

    fn render_room(
        cfg: ViewConfig,
        tex: &dyn Textures,
        smooth: bool,
        angle: i32,
        pos: (i32, i32),
    ) -> Framebuffer {
        let mut v = View3D::new(cfg);
        let mut fb = Framebuffer::new(cfg.width, cfg.height());
        // A colour no texture or shade produces: shows undrawn pixels.
        fb.clear(UNDRAWN);
        render_into(&mut v, &mut fb, tex, smooth, angle, pos);
        fb
    }

    const UNDRAWN: u8 = 250;

    fn render_into(
        v: &mut View3D,
        fb: &mut Framebuffer,
        tex: &dyn Textures,
        smooth: bool,
        angle: i32,
        pos: (i32, i32),
    ) {
        let (tiles, info, plane0) = room();
        let table = build_shade_table(false);
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
            player_x: pos.0,
            player_y: pos.1,
            player_angle: angle,
            player_fine_angle: None,
            sprites: &[],
            weapon: None,
            smooth_floors: smooth,
        };
        fb.begin_frame();
        v.render(fb, 0, 0, &scene, tex);
    }

    /// Lower-half pixels left with the background's floor colours (115–127).
    fn background_in_floor(fb: &Framebuffer) -> usize {
        (fb.height / 2..fb.height)
            .flat_map(|y| (0..fb.width).map(move |x| (x, y)))
            .filter(|&(x, y)| (112..=127).contains(&fb.get(x, y)))
            .count()
    }

    #[test]
    fn smooth_floors_leave_no_gaps() {
        let cfg = ViewConfig { viewsize: 18, k: 3, width: 1200 };
        let tex = StripeTex::new(1);
        for (angle, pos) in
            [(0, (32 << 16, 32 << 16)), (37, (10 << 16, 50 << 16)), (200, (0x1C8000, 0x2A4000))]
        {
            let original = render_room(cfg, &tex, false, angle, pos);
            assert!(background_in_floor(&original) > 0, "the original has grout gaps");
            let smooth = render_room(cfg, &tex, true, angle, pos);
            assert_eq!(background_in_floor(&smooth), 0, "angle {angle}: no gaps");
            let undrawn = smooth.pixels.iter().filter(|&&p| p == UNDRAWN).count();
            assert_eq!(undrawn, 0, "angle {angle}: every pixel drawn without the background");
            assert!(smooth.is_mixed(), "the shade blends between steps");
        }
    }

    #[test]
    fn classic_ignores_enhanced_options() {
        let cfg = ViewConfig::classic(18);
        for angle in [0, 45, 135, 300] {
            let pos = (0x1C8000, 0x2A4000);
            let plain = render_room(cfg, &StripeTex::new(1), false, angle, pos);
            let enhanced = render_room(cfg, &StripeTex::new(2), true, angle, pos);
            assert_eq!(plain.pixels, enhanced.pixels, "angle {angle}");
            assert!(!enhanced.is_mixed());
        }
    }

    #[test]
    fn upscaled_walls_blend_texels() {
        let cfg = ViewConfig { viewsize: 18, k: 4, width: 1152 };
        let pos = (40 << 16, 3 << 16);
        let plain = render_room(cfg, &StripeTex::new(1), false, 90, pos);
        assert!(!plain.is_mixed());
        for n in [2, 4, 6] {
            let hi = render_room(cfg, &StripeTex::new(n), false, 90, pos);
            assert!(hi.is_mixed(), "{n}x: edge texels blend");
            // Same geometry: the wall covers the same rows in the centre column.
            let c = cfg.width / 2;
            let wall = |fb: &Framebuffer| {
                (0..fb.height).filter(|&y| (16..=63).contains(&fb.get(c, y))).count()
            };
            assert_eq!(wall(&plain), wall(&hi), "{n}x");
        }
    }

    #[test]
    #[ignore]
    fn bench_render() {
        let cfg = ViewConfig { viewsize: 18, k: 8, width: 3412 };
        let mut v = View3D::new(cfg);
        let mut fb = Framebuffer::new(cfg.width, cfg.height());
        for (n, smooth) in [(1, false), (1, true), (4, false), (4, true), (6, true)] {
            let tex = StripeTex::new(n);
            render_into(&mut v, &mut fb, &tex, smooth, 37, (10 << 16, 50 << 16));
            let t = std::time::Instant::now();
            for i in 0..20 {
                render_into(&mut v, &mut fb, &tex, smooth, 37 + i, (0x1C8000, 0x2A4000));
            }
            eprintln!("n {n} smooth {smooth}: {:.2} ms/frame", t.elapsed().as_secs_f64() * 50.0);
        }
    }

    #[test]
    fn smooth_shade_curve_follows_the_table() {
        for dark in [false, true] {
            let t = build_shade_table(dark);
            let curve = smooth_shades(&t);
            for w in curve.windows(2) {
                assert!(w[1] <= w[0] + 1e-4, "never brighter far away");
            }
            // In the middle of each unit step the curve rounds to the table
            // value (the first steps, at the horizon, are one or two entries
            // long and jump by more).
            let mut start = 0;
            for i in 1..=256 {
                if i == 256 || t[i] != t[start] {
                    let mid = (start + i - 1) / 2;
                    let unit = start > 0
                        && i < 256
                        && i - start >= 3
                        && t[start - 1] == t[start] + 1
                        && t[start] == t[i] + 1;
                    if unit {
                        assert_eq!(curve[mid].round() as u8, t[mid], "dark {dark}, index {mid}");
                    }
                    start = i;
                }
            }
        }
    }
}
