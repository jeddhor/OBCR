//! The 3D view: projection setup, ray tracing, wall posts, sprites and the
//! player weapon.
//!
//! Port of `CalcProjection`/`SetupScaling` (`WOLFSRC/WL_MAIN.C`,
//! `WL_SCALE.C`), `AsmRefresh` (`WL_DR_A.ASM`), `WallRefresh`, `HitVertWall`
//! and friends, `CalcHeight`, `ScalePost`, `TransformActor`,
//! `TransformTile`, `CalcRotate`, `DrawScaleds` and `DrawPlayerWeapon`
//! (`WL_DRAW.C`), and `ScaleShape`/`SimpleScaleShape` (`WL_SCALE.C`).
//! DELTA-UNCHECKED: OBC deltas from docs/re/RENDERER.md are applied as they
//! are confirmed; tile semantics come from the caller ([`Cell`]).

use obc_core::fixed::fixed_by_frac;
use obc_formats::vswap::Shape;

use crate::framebuffer::Framebuffer;
use crate::tables::{FINEANGLES, Tables, radtoint};

const TILEGLOBAL: i32 = 1 << 16;
const FOCALLENGTH: i32 = 0x5700;
const MINDIST: i32 = 0x5800;
const VIEWGLOBAL: i64 = 0x10000;
const ACTORSIZE: i32 = 0x4000;
const DEG90: i32 = 900;
const DEG180: i32 = 1800;
const DEG270: i32 = 2700;
const DEG360: i32 = 3600;
const MAXVISIBLE: usize = 50;

/// One map cell as the renderer needs it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Cell {
    #[default]
    Empty,
    /// Solid wall: GFXTILES pages for horizontal (north/south) and vertical (east/west) faces.
    Wall { horiz: u16, vert: u16 },
    /// Sliding door: leading-edge position (0 = closed … 0xFFFF open) and pages.
    Door { position: u16, horiz: u16, vert: u16 },
    /// The moving push wall (see [`Scene::pwall_pos`]).
    PushWall { horiz: u16, vert: u16 },
}

/// Floor and ceiling treatment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FloorCeiling {
    /// Wolf3D `VGAClearScreen`: top half ceiling color, bottom half floor color.
    Solid { ceiling: u8, floor: u8 },
}

/// A drawable object.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SpriteRef {
    /// World position (16.16).
    pub x: i32,
    pub y: i32,
    /// Tile for visibility tests.
    pub tile_x: u8,
    pub tile_y: u8,
    /// Sprite index (0-based from the GFXTILES sprite start).
    pub shape: u16,
    /// Statics are drawn at their tile center; actors at their position.
    pub is_static: bool,
    /// Directional sprites: `rotate` (0 none, 1 eight-way, 2 pain) and facing angle (degrees).
    pub rotate: u8,
    pub facing: i32,
}

/// What to draw.
pub struct Scene<'a> {
    /// 64×64 cells, indexed `x * 64 + y` like `tilemap`.
    pub cells: &'a [Cell],
    /// Push-wall travel (0..63) when a push wall is moving.
    pub pwall_pos: u16,
    /// Door-jamb pages drawn on walls beside doors (horizontal face, vertical face).
    pub jamb: (u16, u16),
    pub floor_ceiling: FloorCeiling,
    /// Player position (16.16) and angle in degrees.
    pub player_x: i32,
    pub player_y: i32,
    pub player_angle: i32,
    pub sprites: &'a [SpriteRef],
    /// Player weapon sprite, if any.
    pub weapon: Option<u16>,
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
    pub width: usize,
    pub height: usize,
    /// Width spanned by the original field of view. Equal to `width` for the
    /// original aspect; smaller for widescreen (Hor+: extra view to the sides).
    pub fov_width: usize,
    /// Reproduce the original's scaler quirks (shared scalers above
    /// `stepbytwo`, `maxscale` clamping). Classic only.
    pub classic: bool,
}

impl ViewConfig {
    /// Original view of `width × height` pixels.
    pub fn classic(width: usize, height: usize) -> Self {
        Self { width, height, fov_width: width, classic: true }
    }
}

/// Column scaler for one post height (`BuildCompScale`).
#[derive(Debug, Clone)]
struct Scaler {
    /// Screen rows `[start, end)` for each source texel, before clipping.
    rows: [(i32, i32); 64],
    /// Screen columns covered by each source column (horizontal sprite scaling).
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

/// Projection and per-frame working storage.
pub struct View3D {
    cfg: ViewConfig,
    tables: Tables,
    centerx: i32,
    scale: i64,
    heightnumerator: i64,
    pixelangle: Vec<i32>,
    /// Index of the scaler for each height index (`scaledirectory`).
    scaler_for: Vec<usize>,
    scalers: Vec<Scaler>,
    maxscale: i32,
    wallheight: Vec<i32>,
    /// Tiles a ray passed through this frame (`spotvis`).
    pub spotvis: Vec<bool>,
}

struct Ray {
    viewx: i32,
    viewy: i32,
    viewsin: i32,
    viewcos: i32,
}

struct PostState {
    postx: usize,
    postwidth: usize,
    page: u16,
    column: u16,
    lastside: i32,
    lastintercept: i32,
    lasttile: (u8, i32),
}

impl View3D {
    pub fn new(cfg: ViewConfig) -> Self {
        let tables = Tables::build();
        let mut v = Self {
            cfg,
            tables,
            centerx: 0,
            scale: 0,
            heightnumerator: 0,
            pixelangle: Vec::new(),
            scaler_for: Vec::new(),
            scalers: Vec::new(),
            maxscale: 0,
            wallheight: vec![0; cfg.width],
            spotvis: vec![false; 64 * 64],
        };
        v.calc_projection();
        v.setup_scaling();
        v
    }

    pub fn config(&self) -> ViewConfig {
        self.cfg
    }

    /// `CalcProjection(FOCALLENGTH)`, generalized: angles and scale come from
    /// `fov_width`; columns beyond it (widescreen) continue at the same density.
    fn calc_projection(&mut self) {
        let width = self.cfg.width as i64;
        let fov = self.cfg.fov_width as i64;
        let facedist = f64::from(FOCALLENGTH + MINDIST);
        let halffov = fov / 2;
        self.centerx = (width / 2 - 1) as i32;
        self.scale = (halffov as f64 * facedist / (VIEWGLOBAL / 2) as f64) as i64;
        self.heightnumerator = (i64::from(TILEGLOBAL) * self.scale) >> 6;
        let halfview = (width / 2) as usize;
        let r = radtoint();
        self.pixelangle = vec![0; self.cfg.width];
        for i in 0..halfview {
            let tang = (i as i64 * VIEWGLOBAL / fov) as f64 / facedist;
            let angle = f64::from(tang.atan() as f32);
            let intang = (angle * r) as i32;
            self.pixelangle[halfview - 1 - i] = intang;
            self.pixelangle[halfview + i] = -intang;
        }
    }

    /// `SetupScaling(viewwidth*1.5)`: one scaler per two pixels of height.
    fn setup_scaling(&mut self) {
        let vh = self.cfg.height as i32;
        let maxscaleheight = if self.cfg.classic {
            (self.cfg.width as i32 * 3 / 2) / 2
        } else {
            // Close walls in high resolution: allow posts up to 16× the view height.
            vh * 8
        };
        self.maxscale = maxscaleheight - 1;
        let stepbytwo = vh / 2;
        self.scalers.clear();
        self.scaler_for = vec![0; maxscaleheight as usize + 3];
        let mut i = 1;
        while i <= maxscaleheight {
            self.scalers.push(Scaler::build(i * 2, vh));
            let idx = self.scalers.len() - 1;
            self.scaler_for[i as usize] = idx;
            if self.cfg.classic && i >= stepbytwo {
                // Shared by the next two heights (the original's memory saving).
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

    /// Renders `scene` into `fb` at (`x0`, `y0`) (the view window origin).
    pub fn render(
        &mut self,
        fb: &mut Framebuffer,
        x0: i32,
        y0: i32,
        scene: &Scene,
        tex: &dyn Textures,
    ) {
        self.clear_screen(fb, x0, y0, scene.floor_ceiling);
        let ray = self.wall_refresh(fb, x0, y0, scene, tex);
        self.draw_scaleds(fb, x0, y0, scene, tex, &ray);
        if let Some(shape) = scene.weapon {
            self.draw_player_weapon(fb, x0, y0, shape, tex);
        }
    }

    fn clear_screen(&self, fb: &mut Framebuffer, x0: i32, y0: i32, fc: FloorCeiling) {
        let (w, h) = (self.cfg.width as i32, self.cfg.height as i32);
        match fc {
            FloorCeiling::Solid { ceiling, floor } => {
                fb.bar(x0, y0, w, h / 2, ceiling);
                fb.bar(x0, y0 + h / 2, w, h - h / 2, floor);
            }
        }
    }

    /// `WallRefresh` + `AsmRefresh`.
    fn wall_refresh(
        &mut self,
        fb: &mut Framebuffer,
        x0: i32,
        y0: i32,
        scene: &Scene,
        tex: &dyn Textures,
    ) -> Ray {
        let viewangle = scene.player_angle.rem_euclid(360) as usize;
        let midangle = viewangle as i32 * (FINEANGLES / 360);
        let viewsin = self.tables.sin(viewangle);
        let viewcos = self.tables.cos(viewangle);
        let viewx = scene.player_x - fixed_by_frac(FOCALLENGTH, viewcos);
        let viewy = scene.player_y + fixed_by_frac(FOCALLENGTH, viewsin);
        let focaltx = viewx >> 16;
        let focalty = viewy >> 16;
        let xpartialdown = (viewx & 0xFFFF) as u16;
        let xpartialup = (TILEGLOBAL - i32::from(xpartialdown)) as u16;
        let ypartialdown = (viewy & 0xFFFF) as u16;
        let ypartialup = (TILEGLOBAL - i32::from(ypartialdown)) as u16;
        let ray = Ray { viewx, viewy, viewsin, viewcos };

        self.spotvis.fill(false);
        let mut post = PostState {
            postx: 0,
            postwidth: 0,
            page: 0,
            column: 0,
            lastside: -1,
            lastintercept: 0,
            lasttile: (0, -1),
        };
        let cell_at = |x: i32, y: i32| -> Cell {
            if (0..64).contains(&x) && (0..64).contains(&y) {
                scene.cells[(x * 64 + y) as usize]
            } else {
                Cell::Wall { horiz: 0, vert: 0 }
            }
        };
        let ft = self.tables.finetangent.clone();

        for pixx in 0..self.cfg.width {
            let mut angle = midangle + self.pixelangle[pixx];
            if angle < 0 {
                angle += FINEANGLES;
            }
            if angle >= DEG360 {
                angle -= FINEANGLES;
            }
            let (xtilestep, ytilestep, xstep, ystep, xpartial, ypartial) = if angle < DEG90 {
                (
                    1,
                    -1,
                    ft[(DEG90 - 1 - angle) as usize],
                    -ft[angle as usize],
                    xpartialup,
                    ypartialdown,
                )
            } else if angle < DEG180 {
                (
                    -1,
                    -1,
                    -ft[(angle - DEG90) as usize],
                    -ft[(DEG180 - 1 - angle) as usize],
                    xpartialdown,
                    ypartialdown,
                )
            } else if angle < DEG270 {
                (
                    -1,
                    1,
                    -ft[(DEG270 - 1 - angle) as usize],
                    ft[(angle - DEG180) as usize],
                    xpartialdown,
                    ypartialup,
                )
            } else {
                (
                    1,
                    1,
                    ft[(angle - DEG270) as usize],
                    ft[(DEG360 - 1 - angle) as usize],
                    xpartialup,
                    ypartialup,
                )
            };
            let mut yintercept = viewy.wrapping_add(partial_mul(ystep, xpartial));
            let mut xtile = focaltx + xtilestep;
            let mut xintercept = viewx.wrapping_add(partial_mul(xstep, ypartial));
            let mut ytile = focalty + ytilestep;

            // The two-way trace of AsmRefresh: `vert` checks the next vertical
            // grid line, `horiz` the next horizontal one.
            let mut check_vert = true;
            let hit: Hit = loop {
                if check_vert {
                    let yint = yintercept >> 16;
                    let go_horiz = if ytilestep == -1 { yint <= ytile } else { yint >= ytile };
                    if go_horiz {
                        check_vert = false;
                        continue;
                    }
                    // vertentry
                    match cell_at(xtile, yint) {
                        Cell::Empty => {}
                        Cell::Door { position, .. } => {
                            let half = yintercept.wrapping_add(ystep >> 1);
                            if half >> 16 == yint && (half & 0xFFFF) as u16 >= position {
                                break Hit::Door {
                                    vertical: true,
                                    x: xtile,
                                    y: yint,
                                    xi: (xtile << 16) | 0x8000,
                                    yi: half,
                                    position,
                                };
                            }
                        }
                        Cell::PushWall { .. } => {
                            let off =
                                (ystep as u32).wrapping_mul(u32::from(scene.pwall_pos)) as i32 >> 6;
                            let np = yintercept.wrapping_add(off);
                            if np >> 16 == yint {
                                break Hit::PushWall {
                                    vertical: true,
                                    x: xtile,
                                    y: yint,
                                    xi: xtile << 16,
                                    yi: np,
                                };
                            }
                        }
                        Cell::Wall { .. } => {
                            break Hit::Wall {
                                vertical: true,
                                x: xtile,
                                y: yint,
                                xi: xtile << 16,
                                yi: yintercept,
                            };
                        }
                    }
                    // passvert
                    if (0..64).contains(&xtile) && (0..64).contains(&yint) {
                        self.spotvis[(xtile * 64 + yint) as usize] = true;
                    }
                    xtile += xtilestep;
                    yintercept = yintercept.wrapping_add(ystep);
                    if !(-1..=64).contains(&xtile) {
                        break Hit::None;
                    }
                } else {
                    let xint = xintercept >> 16;
                    let go_vert = if xtilestep == -1 { xint <= xtile } else { xint >= xtile };
                    if go_vert {
                        check_vert = true;
                        continue;
                    }
                    // horizentry
                    match cell_at(xint, ytile) {
                        Cell::Empty => {}
                        Cell::Door { position, .. } => {
                            let half = xintercept.wrapping_add(xstep >> 1);
                            if half >> 16 == xint && (half & 0xFFFF) as u16 >= position {
                                break Hit::Door {
                                    vertical: false,
                                    x: xint,
                                    y: ytile,
                                    xi: half,
                                    yi: (ytile << 16) | 0x8000,
                                    position,
                                };
                            }
                        }
                        Cell::PushWall { .. } => {
                            let off =
                                (xstep as u32).wrapping_mul(u32::from(scene.pwall_pos)) as i32 >> 6;
                            let np = xintercept.wrapping_add(off);
                            if np >> 16 == xint {
                                break Hit::PushWall {
                                    vertical: false,
                                    x: xint,
                                    y: ytile,
                                    xi: np,
                                    yi: ytile << 16,
                                };
                            }
                        }
                        Cell::Wall { .. } => {
                            break Hit::Wall {
                                vertical: false,
                                x: xint,
                                y: ytile,
                                xi: xintercept,
                                yi: ytile << 16,
                            };
                        }
                    }
                    // passhoriz
                    if (0..64).contains(&xint) && (0..64).contains(&ytile) {
                        self.spotvis[(xint * 64 + ytile) as usize] = true;
                    }
                    ytile += ytilestep;
                    xintercept = xintercept.wrapping_add(xstep);
                    if !(-1..=64).contains(&ytile) {
                        break Hit::None;
                    }
                }
            };
            self.hit(
                fb, x0, y0, pixx, hit, xtilestep, ytilestep, scene, &ray, &mut post, tex, &cell_at,
            );
        }
        if post.lastside != -1 {
            self.scale_post(fb, x0, y0, &post, tex);
        }
        ray
    }

    #[allow(clippy::too_many_arguments)]
    fn hit(
        &mut self,
        fb: &mut Framebuffer,
        x0: i32,
        y0: i32,
        pixx: usize,
        hit: Hit,
        xtilestep: i32,
        ytilestep: i32,
        scene: &Scene,
        ray: &Ray,
        post: &mut PostState,
        tex: &dyn Textures,
        cell_at: &dyn Fn(i32, i32) -> Cell,
    ) {
        // (side, intercept for merging, tile value identity, texture column, page)
        let (side, intercept_key, texture, page, xi, yi, tilekey) = match hit {
            Hit::None => {
                self.wallheight[pixx] = 8;
                return;
            }
            Hit::Wall { vertical: true, x, y, xi, yi } => {
                // HitVertWall
                let mut texture = ((yi >> 4) & 0xFC0) as u16;
                let mut xi = xi;
                if xtilestep == -1 {
                    texture = 0xFC0 - texture;
                    xi += TILEGLOBAL;
                }
                let page = match cell_at(x - xtilestep, y) {
                    Cell::Door { .. } => scene.jamb.1,
                    _ => match cell_at(x, y) {
                        Cell::Wall { vert, .. } => vert,
                        _ => 0,
                    },
                };
                (1, x, texture, page, xi, yi, (1u8, i32::from(page)))
            }
            Hit::Wall { vertical: false, x, y, xi, yi } => {
                // HitHorizWall
                let mut texture = ((xi >> 4) & 0xFC0) as u16;
                let mut yi = yi;
                if ytilestep == -1 {
                    yi += TILEGLOBAL;
                } else {
                    texture = 0xFC0 - texture;
                }
                let page = match cell_at(x, y - ytilestep) {
                    Cell::Door { .. } => scene.jamb.0,
                    _ => match cell_at(x, y) {
                        Cell::Wall { horiz, .. } => horiz,
                        _ => 0,
                    },
                };
                (0, y, texture, page, xi, yi, (0u8, i32::from(page)))
            }
            Hit::Door { vertical, x, y, xi, yi, position } => {
                let along = if vertical { yi } else { xi };
                let texture = ((along.wrapping_sub(i32::from(position)) >> 4) & 0xFC0) as u16;
                let page = match cell_at(x, y) {
                    Cell::Door { horiz, vert, .. } => {
                        if vertical {
                            vert
                        } else {
                            horiz
                        }
                    }
                    _ => 0,
                };
                (2, -1, texture, page, xi, yi, (2u8, x * 64 + y))
            }
            Hit::PushWall { vertical, x, y, xi, yi } => {
                let offset = i32::from(scene.pwall_pos) << 10;
                let (mut xi, mut yi) = (xi, yi);
                let mut texture;
                if vertical {
                    texture = ((yi >> 4) & 0xFC0) as u16;
                    if xtilestep == -1 {
                        xi += TILEGLOBAL - offset;
                    } else {
                        texture = 0xFC0 - texture;
                        xi += offset;
                    }
                } else {
                    texture = ((xi >> 4) & 0xFC0) as u16;
                    if ytilestep == -1 {
                        yi += TILEGLOBAL - offset;
                    } else {
                        texture = 0xFC0 - texture;
                        yi += offset;
                    }
                }
                let page = match cell_at(x, y) {
                    Cell::PushWall { horiz, vert } => {
                        if vertical {
                            vert
                        } else {
                            horiz
                        }
                    }
                    _ => 0,
                };
                (3, -1, texture, page, xi, yi, (3u8, x * 64 + y))
            }
        };
        let height = self.calc_height(xi, yi, ray);
        self.wallheight[pixx] = height;
        let same_wall = post.lastside == side
            && post.lasttile == tilekey
            && (side >= 2 || post.lastintercept == intercept_key);
        if same_wall {
            if texture >> 6 == post.column {
                post.postwidth += 1;
                self.wallheight[pixx] = self.wallheight[pixx - 1];
                return;
            }
            self.scale_post(fb, x0, y0, post, tex);
            post.column = texture >> 6;
            post.postwidth = 1;
            post.postx = pixx;
            return;
        }
        if post.lastside != -1 {
            self.scale_post(fb, x0, y0, post, tex);
        }
        post.lastside = side;
        post.lastintercept = intercept_key;
        post.lasttile = tilekey;
        post.postx = pixx;
        post.postwidth = 1;
        post.page = page;
        post.column = texture >> 6;
    }

    /// `CalcHeight`: `heightnumerator / (nx >> 8)`, at least 8.
    fn calc_height(&self, xintercept: i32, yintercept: i32, ray: &Ray) -> i32 {
        let gxt = fixed_by_frac(xintercept.wrapping_sub(ray.viewx), ray.viewcos);
        let gyt = fixed_by_frac(yintercept.wrapping_sub(ray.viewy), ray.viewsin);
        let nx = (gxt.wrapping_sub(gyt)).max(MINDIST);
        let div = if self.cfg.classic { i64::from((nx >> 8) as i16) } else { i64::from(nx >> 8) };
        let h = (self.heightnumerator / div.max(1)) as i32;
        h.max(8)
    }

    /// `ScalePost`: draws `postwidth` columns of one texture column.
    fn scale_post(
        &self,
        fb: &mut Framebuffer,
        x0: i32,
        y0: i32,
        post: &PostState,
        tex: &dyn Textures,
    ) {
        let Some(wall) = tex.wall(post.page) else { return };
        let mut index = self.wallheight[post.postx] >> 3;
        if index > self.maxscale {
            index = self.maxscale;
        }
        let scaler = self.scaler(index);
        let col = &wall[post.column as usize * 64..post.column as usize * 64 + 64];
        let vh = self.cfg.height as i32;
        for x in post.postx..post.postx + post.postwidth {
            for (src, &(start, end)) in scaler.rows.iter().enumerate() {
                let color = col[src];
                for y in start.max(0)..end.min(vh) {
                    fb.put(x0 + x as i32, y0 + y, color);
                }
            }
        }
    }

    /// `TransformActor`/`TransformTile`: screen x and height, or `None` if too close.
    fn transform(&self, gx: i32, gy: i32, size: i32, ray: &Ray) -> Option<(i32, i32, i32, i32)> {
        let nx = fixed_by_frac(gx, ray.viewcos)
            .wrapping_sub(fixed_by_frac(gy, ray.viewsin))
            .wrapping_sub(size);
        let ny = fixed_by_frac(gy, ray.viewcos).wrapping_add(fixed_by_frac(gx, ray.viewsin));
        if nx < MINDIST {
            return None;
        }
        let viewx = self.centerx + (i64::from(ny) * self.scale / i64::from(nx)) as i32;
        let div = if self.cfg.classic { i64::from((nx >> 8) as i16) } else { i64::from(nx >> 8) };
        let height = (self.heightnumerator / div.max(1)) as i32;
        Some((viewx, height, nx, ny))
    }

    /// `CalcRotate`: rotation frame offset for an eight-way sprite.
    fn calc_rotate(&self, s: &SpriteRef, viewx: i32, player_angle: i32) -> u16 {
        // The original measures screen offset in its own pixels; scale to them.
        let off = (self.centerx - viewx) as i64 * 304 / self.cfg.fov_width.max(1) as i64;
        let viewangle = player_angle + (off / 8) as i32;
        let mut angle = (viewangle - 180) - s.facing + 360 / 16;
        angle = angle.rem_euclid(360);
        if s.rotate == 2 { (4 * (angle / 180)) as u16 } else { (angle / 45) as u16 }
    }

    /// `DrawScaleds`: statics and actors, farthest first.
    fn draw_scaleds(
        &self,
        fb: &mut Framebuffer,
        x0: i32,
        y0: i32,
        scene: &Scene,
        tex: &dyn Textures,
        ray: &Ray,
    ) {
        let mut vis: Vec<(i32, i32, u16)> = Vec::new();
        for s in scene.sprites {
            if vis.len() >= MAXVISIBLE - 1 && self.cfg.classic {
                break;
            }
            let (tx, ty) = (s.tile_x as i32, s.tile_y as i32);
            let seen = |dx: i32, dy: i32| -> bool {
                let (x, y) = (tx + dx, ty + dy);
                (0..64).contains(&x) && (0..64).contains(&y) && self.spotvis[(x * 64 + y) as usize]
            };
            let open = |dx: i32, dy: i32| -> bool {
                let (x, y) = (tx + dx, ty + dy);
                (0..64).contains(&x)
                    && (0..64).contains(&y)
                    && scene.cells[(x * 64 + y) as usize] == Cell::Empty
            };
            let (gx, gy, size) = if s.is_static {
                if !seen(0, 0) {
                    continue;
                }
                ((tx << 16) + 0x8000 - ray.viewx, (ty << 16) + 0x8000 - ray.viewy, 0x2000)
            } else {
                let visible = seen(0, 0)
                    || [(-1, -1), (-1, 0), (-1, 1), (0, -1), (0, 1), (1, -1), (1, 0), (1, 1)]
                        .iter()
                        .any(|&(dx, dy)| seen(dx, dy) && open(dx, dy));
                if !visible {
                    continue;
                }
                (s.x - ray.viewx, s.y - ray.viewy, ACTORSIZE)
            };
            let Some((viewx, height, _, _)) = self.transform(gx, gy, size, ray) else { continue };
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
        for (viewx, height, shape) in vis {
            self.scale_shape(fb, x0, y0, viewx, shape, height, tex);
        }
    }

    /// `ScaleShape`: a world sprite clipped against `wallheight`. Multi-column
    /// spans test only their end columns, as the original does.
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
    ) {
        let Some(sh) = tex.sprite(shape) else { return };
        let scale = height >> 3;
        if scale == 0 || (self.cfg.classic && scale > self.maxscale) {
            return;
        }
        let sc = self.scaler(scale.min(self.maxscale));
        let vw = self.cfg.width as i32;
        let wh = |x: i32| self.wallheight[x as usize];
        let draw = |fb: &mut Framebuffer, x: i32, w: i32, srcx: i32| {
            self.scale_line(fb, x0, y0, x, w, sh, srcx as usize, sc);
        };

        // Left half: source columns 31 down to leftpix, laid out leftward from xcenter.
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

        // Right half: source columns 32 up to rightpix.
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
    ) {
        let vh = self.cfg.height as i32;
        for post in &sh.posts[srcx] {
            for src in post.start as usize..post.end as usize {
                let (start, end) = sc.rows[src];
                let color = sh.pixels[srcx * 64 + src];
                for y in start.max(0)..end.min(vh) {
                    for col in x..x + w {
                        fb.put(x0 + col, y0 + y, color);
                    }
                }
            }
        }
    }

    /// `DrawPlayerWeapon`: `SimpleScaleShape(viewwidth/2, shape, viewheight+1)`,
    /// unclipped, using the shared scaler for index `height >> 1`.
    fn draw_player_weapon(
        &self,
        fb: &mut Framebuffer,
        x0: i32,
        y0: i32,
        shape: u16,
        tex: &dyn Textures,
    ) {
        let Some(sh) = tex.sprite(shape) else { return };
        let sc = self.scaler((self.cfg.height as i32 + 1) >> 1);
        let xcenter = self.cfg.width as i32 / 2;
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
            self.scale_line(fb, x0, y0, slinex, w, sh, srcx as usize, sc);
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
            self.scale_line(fb, x0, y0, slinex, w, sh, srcx as usize, sc);
            slinex += w;
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum Hit {
    None,
    Wall { vertical: bool, x: i32, y: i32, xi: i32, yi: i32 },
    Door { vertical: bool, x: i32, y: i32, xi: i32, yi: i32, position: u16 },
    PushWall { vertical: bool, x: i32, y: i32, xi: i32, yi: i32 },
}

/// `xpartialbyystep`/`ypartialbyxstep`: signed 32-bit step times an unsigned
/// 16-bit fraction, magnitude truncated (`WOLFSRC/WL_DR_A.ASM`).
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
    fn projection_matches_wolf3d_for_full_view() {
        let v = View3D::new(ViewConfig::classic(304, 152));
        // 152 * 44800 / 32768 = 207.8 -> 207; heightnumerator = 65536*207 >> 6.
        assert_eq!(v.scale, 207);
        assert_eq!(v.heightnumerator, 211_968);
        assert_eq!(v.centerx, 151);
        assert_eq!(v.pixelangle[151], -v.pixelangle[152]);
    }

    #[test]
    fn renders_a_closed_room_without_panicking() {
        let mut cells = vec![Cell::Empty; 4096];
        for i in 0..64 {
            for (x, y) in [(i, 0), (i, 63), (0, i), (63, i)] {
                cells[x * 64 + y] = Cell::Wall { horiz: 0, vert: 1 };
            }
        }
        let mut v = View3D::new(ViewConfig::classic(304, 152));
        let mut fb = Framebuffer::new(320, 200);
        for angle in (0..360).step_by(7) {
            let scene = Scene {
                cells: &cells,
                pwall_pos: 0,
                jamb: (0, 0),
                floor_ceiling: FloorCeiling::Solid { ceiling: 1, floor: 2 },
                player_x: 32 << 16,
                player_y: 32 << 16,
                player_angle: angle,
                sprites: &[],
                weapon: None,
            };
            v.render(&mut fb, 8, 4, &scene, &NoTex);
        }
        assert!(v.spotvis.iter().filter(|&&s| s).count() > 100);
    }

    #[test]
    fn partial_mul_signs() {
        assert_eq!(partial_mul(0x20000, 0x8000), 0x10000);
        assert_eq!(partial_mul(-0x20000, 0x8000), -0x10000);
    }
}
