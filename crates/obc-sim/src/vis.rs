//! The gameplay side of `ThreeDRefresh` (ghidra 1b4a:129c): ray pass A fills
//! `spotvis`, then the first `DrawScaleds` activates seen actors, sets
//! `FL_VISABLE`/`viewx`/`transx` for the attack code and calls `GetBonus` for
//! grabbable items (docs/re/RENDERER.md §2, §4.3, §5.1).
//!
//! The simulation always runs this with the Classic view geometry, so
//! presentation choices (high resolution, widescreen) never change gameplay.

use obc_core::fixed::fixed_by_frac;
use obc_core::trig::{FINEANGLES, radtoint};

use crate::player;
use crate::world::{TILEGLOBAL, World, fl};

const FOCALLENGTH: i32 = 0x5700;
const MINDIST: i32 = 0x5800;
const ACTORSIZE: i32 = 0x4000;
const VIEWGLOBAL: i64 = 0x10000;
/// `vislist` entries (Wolf3D 50).
pub const MAXVISIBLE: usize = 75;

/// Classic view geometry (`SetViewSize`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ViewGeom {
    pub viewsize: u16,
    pub width: i32,
    pub height: i32,
    pub centerx: i32,
    pub shootdelta: i32,
    pub scale: i64,
    pub heightnumerator: i64,
    pub pixelangle: Vec<i32>,
}

impl ViewGeom {
    /// `SetViewSize(viewsize*16, viewsize*8)` and `CalcProjection(FOCALLENGTH)`.
    pub fn new(viewsize: u16) -> Self {
        let n = i32::from(viewsize.clamp(4, 20));
        let width = (n * 16) & !15;
        let height = (n * 8) & !1;
        let facedist = f64::from(FOCALLENGTH + MINDIST);
        let halfview = width / 2;
        let scale = (f64::from(halfview) * facedist / (VIEWGLOBAL / 2) as f64) as i64;
        let heightnumerator = (i64::from(TILEGLOBAL) * scale) >> 6;
        let r = radtoint();
        let mut pixelangle = vec![0; width as usize];
        for i in 0..halfview {
            let tang = (i64::from(i) * VIEWGLOBAL / i64::from(width)) as f64 / facedist;
            let intang = (f64::from(tang.atan() as f32) * r) as i32;
            pixelangle[(halfview - 1 - i) as usize] = intang;
            pixelangle[(halfview + i) as usize] = -intang;
        }
        Self {
            viewsize,
            width,
            height,
            centerx: width / 2 - 1,
            shootdelta: width / 10,
            scale,
            heightnumerator,
            pixelangle,
        }
    }
}

/// Plane-1 see-through markers (103–107).
pub fn is_see_through(info: u16) -> bool {
    (0x67..=0x6B).contains(&(info & 0xFF))
}

/// Plane-1 moving push-wall markers (98, 101, 102).
pub fn is_pushwall_marker(info: u16) -> bool {
    matches!(info & 0xFF, 0x62 | 0x65 | 0x66)
}

struct Ray {
    viewx: i32,
    viewy: i32,
    viewsin: i32,
    viewcos: i32,
}

impl World {
    /// Ray pass A plus the bookkeeping half of `DrawScaleds`.
    pub fn visibility_pass(&mut self) {
        let ray = self.trace_spotvis();
        self.scan_scaleds(&ray);
    }

    /// Ray pass A: `spotvis` for every cell a column's ray passes through.
    fn trace_spotvis(&mut self) -> Ray {
        let p = *self.plr();
        let viewangle = (i32::from(p.angle)).rem_euclid(360) as usize;
        let midangle = viewangle as i32 * (FINEANGLES / 360);
        let viewsin = self.trig.sin(viewangle);
        let viewcos = self.trig.cos(viewangle);
        let viewx = p.x - fixed_by_frac(FOCALLENGTH, viewcos);
        let viewy = p.y + fixed_by_frac(FOCALLENGTH, viewsin);
        let ray = Ray { viewx, viewy, viewsin, viewcos };
        self.spotvis.fill(false);
        let focaltx = viewx >> 16;
        let focalty = viewy >> 16;
        let xpartialdown = (viewx & 0xFFFF) as u16;
        let xpartialup = (xpartialdown as i32).wrapping_neg() as u16;
        let ypartialdown = (viewy & 0xFFFF) as u16;
        let ypartialup = (ypartialdown as i32).wrapping_neg() as u16;
        let ft = &self.trig.finetangent;
        let mut marks: Vec<usize> = Vec::with_capacity(512);
        for &pa in &self.view.pixelangle {
            let mut angle = midangle + pa;
            if angle < 0 {
                angle += FINEANGLES;
            }
            if angle >= FINEANGLES {
                angle -= FINEANGLES;
            }
            let (xs, ys, xstep, ystep, xpartial, ypartial) = if angle < 900 {
                (1, -1, ft[(899 - angle) as usize], -ft[angle as usize], xpartialup, ypartialdown)
            } else if angle < 1800 {
                (
                    -1,
                    -1,
                    -ft[(angle - 900) as usize],
                    -ft[(1799 - angle) as usize],
                    xpartialdown,
                    ypartialdown,
                )
            } else if angle < 2700 {
                (
                    -1,
                    1,
                    -ft[(2699 - angle) as usize],
                    ft[(angle - 1800) as usize],
                    xpartialdown,
                    ypartialup,
                )
            } else {
                (
                    1,
                    1,
                    ft[(angle - 2700) as usize],
                    ft[(3599 - angle) as usize],
                    xpartialup,
                    ypartialup,
                )
            };
            let mut yint = viewy.wrapping_add(partial_mul(ystep, xpartial));
            let mut xtile = focaltx + xs;
            let mut xint = viewx.wrapping_add(partial_mul(xstep, ypartial));
            let mut ytile = focalty + ys;
            let mut vert = true;
            for _ in 0..256 {
                if vert {
                    let yt = yint >> 16;
                    if (ys == -1 && yt <= ytile) || (ys == 1 && yt >= ytile) {
                        vert = false;
                        continue;
                    }
                    if !World::in_map(xtile, yt) {
                        break;
                    }
                    if self.cell_stops(xtile, yt, yint, ystep) {
                        break;
                    }
                    marks.push(World::idx(xtile, yt));
                    xtile += xs;
                    yint = yint.wrapping_add(ystep);
                } else {
                    let xt = xint >> 16;
                    if (xs == -1 && xt <= xtile) || (xs == 1 && xt >= xtile) {
                        vert = true;
                        continue;
                    }
                    if !World::in_map(xt, ytile) {
                        break;
                    }
                    if self.cell_stops(xt, ytile, xint, xstep) {
                        break;
                    }
                    marks.push(World::idx(xt, ytile));
                    ytile += ys;
                    xint = xint.wrapping_add(xstep);
                }
            }
        }
        for i in marks {
            self.spotvis[i] = true;
        }
        ray
    }

    /// True if the ray stops in this cell (wall, door leaf or push wall hit).
    /// `along`/`step` are the intercept and step along the cell's axis.
    fn cell_stops(&self, x: i32, y: i32, along: i32, step: i32) -> bool {
        let info = self.mapspot(x, y, 1);
        if is_see_through(info) {
            return false;
        }
        let t = self.tile(x, y);
        if t == 0 {
            return false;
        }
        if t & 0x80 == 0 {
            return true;
        }
        if is_pushwall_marker(info) {
            let off = (step as u32).wrapping_mul(u32::from(self.pwall.pos)) as i32 >> 6;
            return along.wrapping_add(off) >> 16 == along >> 16;
        }
        let door = usize::from(t & 0x7F);
        let half = along.wrapping_add(step >> 1);
        let pos = self.doors.get(door).map_or(0, |d| d.position);
        half >> 16 == along >> 16 && (half & 0xFFFF) as u16 >= pos
    }

    /// `TransformTile`/`TransformActor` core: `(nx, ny, Some((viewx, viewheight)))`,
    /// or `None` for the projection when closer than `MINDIST`.
    fn transform(&self, gx: i32, gy: i32, size: i32, ray: &Ray) -> (i32, i32, Option<(i32, i32)>) {
        let nx = fixed_by_frac(gx, ray.viewcos)
            .wrapping_sub(fixed_by_frac(gy, ray.viewsin))
            .wrapping_sub(size);
        let ny = fixed_by_frac(gy, ray.viewcos).wrapping_add(fixed_by_frac(gx, ray.viewsin));
        if nx < MINDIST {
            return (nx, ny, None);
        }
        let viewx = self.view.centerx + (i64::from(ny) * self.view.scale / i64::from(nx)) as i32;
        let height = (self.view.heightnumerator / i64::from((nx >> 8) as i16).max(1)) as i32;
        (nx, ny, Some((viewx, height)))
    }

    /// First `DrawScaleds` call: GetBonus for grabbable items; actor activation and visibility.
    fn scan_scaleds(&mut self, ray: &Ray) {
        for i in 0..self.statics.len() {
            let s = self.statics[i];
            if s.shape == -1 || !self.spotvis[World::idx(i32::from(s.tilex), i32::from(s.tiley))] {
                continue;
            }
            let gx = (i32::from(s.tilex) << 16) + 0x8000 - ray.viewx;
            let gy = (i32::from(s.tiley) << 16) + 0x8000 - ray.viewy;
            let (nx, ny, proj) = self.transform(gx, gy, 0x2000, ray);
            if proj.is_some()
                && nx < TILEGLOBAL
                && ny > -TILEGLOBAL / 2
                && ny < TILEGLOBAL / 2
                && s.flags & fl::BONUS != 0
            {
                player::get_bonus(self, i);
            }
        }
        let mut cur = self.plr().next;
        while let Some(id) = cur {
            cur = self.obj(id).next;
            let o = *self.obj(id);
            let shape = self.data.state(o.state).map_or(0, |s| s.shape);
            if shape == 0 {
                continue;
            }
            let (tx, ty) = (i32::from(o.tilex), i32::from(o.tiley));
            let seen = |w: &World, dx: i32, dy: i32| {
                let (x, y) = (tx + dx, ty + dy);
                World::in_map(x, y) && w.spotvis[World::idx(x, y)]
            };
            let open = |w: &World, dx: i32, dy: i32| w.tile(tx + dx, ty + dy) == 0;
            let visible = seen(self, 0, 0)
                || [(-1, 0), (1, 0), (-1, -1), (0, -1), (1, -1), (-1, 1), (0, 1), (1, 1)]
                    .iter()
                    .any(|&(dx, dy)| seen(self, dx, dy) && open(self, dx, dy));
            if !visible {
                self.obj_mut(id).flags &= !fl::VISABLE;
                continue;
            }
            self.obj_mut(id).active = 1;
            let gx = o.x.wrapping_sub(ray.viewx);
            let gy = o.y.wrapping_sub(ray.viewy);
            let (nx, ny, proj) = self.transform(gx, gy, ACTORSIZE, ray);
            let ob = self.obj_mut(id);
            ob.transx = nx;
            ob.transy = ny;
            match proj {
                Some((vx, h)) => {
                    ob.viewx = vx as i16;
                    ob.viewheight = h as i16;
                    if h != 0 {
                        ob.flags |= fl::VISABLE;
                    }
                }
                None => ob.viewheight = 0,
            }
        }
    }
}

/// `xpartialbyystep`: signed step times an unsigned 16-bit fraction.
fn partial_mul(step: i32, partial: u16) -> i32 {
    let r = ((u64::from(step.unsigned_abs()) * u64::from(partial)) >> 16) as u32 as i32;
    if step < 0 { r.wrapping_neg() } else { r }
}
