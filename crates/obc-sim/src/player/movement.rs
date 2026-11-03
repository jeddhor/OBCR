//! Player movement and collision: `ControlMovement`, `Thrust`, `ClipMove`,
//! `TryMove` (`WOLFSRC/WL_AGENT.C:149`, `:928`, `:866`, `:801`).

use obc_core::fixed::{MINDIST, fixed_by_frac};

use super::{items, trig_index};
use crate::world::{AREATILE, MAPSIZE, MINACTORDIST, NUMAREAS, Obj, ObjId, Occ, World, bt, fl};

/// `PLAYERSIZE` (= `MINDIST`, `WOLFSRC/WL_DEF.H:88`; OBC 0x5800 at 208c:0a08).
pub const PLAYERSIZE: i32 = MINDIST;
/// `MOVESCALE` (`WOLFSRC/WL_AGENT.C:18`; OBC 0x96 at 208c:02a3).
const MOVESCALE: i32 = 150;
/// `BACKMOVESCALE` (`WOLFSRC/WL_AGENT.C:19`; OBC 0x64 at 208c:037a).
const BACKMOVESCALE: i32 = 100;
/// `ANGLESCALE` (`WOLFSRC/WL_AGENT.C:20`; OBC 0x14 at 208c:0300).
const ANGLESCALE: i16 = 20;
/// `ANGLES`.
const ANGLES: i16 = 360;

/// `ControlMovement(ob)` (ghidra 208c:024a, CONFIRMED; `WOLFSRC/WL_AGENT.C:149`).
///
/// Deltas: strafe is `buttonstate[13]`; no `victoryflag` test. Quirk: when
/// `controly == 0` the code still calls `Thrust(di, 0)` (the `jle` at
/// 208c:035e lands in the backward-move call), so the zero-length move, its
/// `TryMove` and the area bookkeeping run every frame. The angle register is
/// stale there, but a zero speed makes it irrelevant.
#[doc(alias = "ControlMovement")]
pub(super) fn control_movement(w: &mut World, id: ObjId) {
    w.thrustspeed = 0;
    let (oldx, oldy) = (w.plr().x, w.plr().y);
    // Stand-in for register `di`: the strafe angle, or the backward angle.
    let mut angle = w.obj(id).angle;

    if w.buttonstate[bt::STRAFE] {
        if w.controlx > 0 {
            angle = w.obj(id).angle.wrapping_sub(ANGLES / 4);
            if angle < 0 {
                angle += ANGLES;
            }
            let speed = i32::from(w.controlx).wrapping_mul(MOVESCALE);
            thrust(w, angle, speed); // move to left
        } else if w.controlx < 0 {
            angle = w.obj(id).angle.wrapping_add(ANGLES / 4);
            if angle >= ANGLES {
                angle -= ANGLES;
            }
            let speed = i32::from(w.controlx.wrapping_neg()).wrapping_mul(MOVESCALE);
            thrust(w, angle, speed); // move to right
        }
    } else {
        w.anglefrac = w.anglefrac.wrapping_add(w.controlx);
        let angleunits = w.anglefrac / ANGLESCALE;
        w.anglefrac = w.anglefrac.wrapping_sub(angleunits.wrapping_mul(ANGLESCALE));
        let o = w.obj_mut(id);
        o.angle = o.angle.wrapping_sub(angleunits);
        if o.angle >= ANGLES {
            o.angle -= ANGLES;
        }
        if o.angle < 0 {
            o.angle += ANGLES;
        }
    }

    if w.controly < 0 {
        let speed = i32::from(w.controly.wrapping_neg()).wrapping_mul(MOVESCALE);
        thrust(w, w.obj(id).angle, speed); // move forwards
    } else {
        if w.controly > 0 {
            angle = w.obj(id).angle.wrapping_add(ANGLES / 2);
            if angle >= ANGLES {
                angle -= ANGLES;
            }
        }
        let speed = i32::from(w.controly).wrapping_mul(BACKMOVESCALE);
        thrust(w, angle, speed); // move backwards (or the zero move, see above)
    }

    let p = *w.plr();
    w.ply.playerxmove = p.x.wrapping_sub(oldx);
    w.ply.playerymove = p.y.wrapping_sub(oldy);
}

/// `Thrust(angle, speed)` (ghidra 208c:0dc7, CONFIRMED; `WOLFSRC/WL_AGENT.C:928`).
///
/// Delta: no `EXITTILE`/`VictoryTile` test; instead, when the new plane-0
/// area differs from the player's and is below 22, `areabyplayer[old]` is
/// cleared and `areabyplayer[new]` set (208c:0e97, docs/re/LEVEL_CODES.md §3.6).
#[doc(alias = "Thrust")]
pub(super) fn thrust(w: &mut World, angle: i16, speed: i32) {
    w.thrustspeed = w.thrustspeed.wrapping_add(speed);
    let mut speed = speed;
    if speed >= MINDIST * 2 {
        speed = MINDIST * 2 - 1;
    }
    let a = trig_index(angle);
    let xmove = fixed_by_frac(speed, w.trig.cos(a));
    let ymove = fixed_by_frac(speed, w.trig.sin(a)).wrapping_neg();
    let p = w.player;
    clip_move(w, p, xmove, ymove);

    let (tx, ty, old_area) = {
        let o = w.obj_mut(p);
        o.tilex = (o.x >> 16) as u16;
        o.tiley = (o.y >> 16) as u16;
        (i32::from(o.tilex as i16), i32::from(o.tiley as i16), o.areanumber)
    };
    let area = w.mapspot(tx, ty, 0).wrapping_sub(AREATILE);
    if u16::from(old_area) != area && area < NUMAREAS as u16 {
        if let Some(a) = w.areabyplayer.get_mut(old_area as usize) {
            *a = false;
        }
        w.areabyplayer[area as usize] = true;
        w.obj_mut(p).areanumber = area as u8;
    }
}

/// `ClipMove(ob, xmove, ymove)` (ghidra 208c:0cbc, CONFIRMED by instruction
/// read; `WOLFSRC/WL_AGENT.C:866`): try the full move, then x only, then y
/// only; `noclip` walks through walls inside the map border. No delta.
#[doc(alias = "ClipMove")]
pub(super) fn clip_move(w: &mut World, id: ObjId, xmove: i32, ymove: i32) {
    let (basex, basey) = (w.obj(id).x, w.obj(id).y);

    set_pos(w, id, basex.wrapping_add(xmove), basey.wrapping_add(ymove));
    if try_move(w, id) {
        return;
    }

    let o = *w.obj(id);
    let edge = (MAPSIZE as i32 - 1) << 16;
    if w.noclip && o.x > 2 << 16 && o.y > 2 << 16 && o.x < edge && o.y < edge {
        return; // walk through walls
    }

    set_pos(w, id, basex.wrapping_add(xmove), basey);
    if try_move(w, id) {
        return;
    }

    set_pos(w, id, basex, basey.wrapping_add(ymove));
    if try_move(w, id) {
        return;
    }

    set_pos(w, id, basex, basey);
}

fn set_pos(w: &mut World, id: ObjId, x: i32, y: i32) {
    let o = w.obj_mut(id);
    o.x = x;
    o.y = y;
}

/// `TryMove(ob)` (ghidra 208c:09f7, CONFIRMED; `WOLFSRC/WL_AGENT.C:801`).
///
/// Deltas: a blocking wall tile 0x49–0x4C hurts the player when
/// *IsFacingTile* says he faces it: 1/5 chance of a visor splat (type 1,
/// with sound), then `TakeDamage(5, NULL)` (barbed wire and the like; it
/// fires on every blocked attempt). Actors with `FL_NONMARK` or `flags2`
/// bit 1 (`fl::NOBLOCK`) never block. The debug messages (DS:0xC000) are
/// not ported.
#[doc(alias = "TryMove")]
pub(super) fn try_move(w: &mut World, id: ObjId) -> bool {
    let o = *w.obj(id);
    let mut xl = o.x.wrapping_sub(PLAYERSIZE) >> 16;
    let mut yl = o.y.wrapping_sub(PLAYERSIZE) >> 16;
    let mut xh = o.x.wrapping_add(PLAYERSIZE) >> 16;
    let mut yh = o.y.wrapping_add(PLAYERSIZE) >> 16;

    // Check for solid walls (actorat is a wall/door marker, not an object).
    for y in yl..=yh {
        for x in xl..=xh {
            if let Occ::Mark(_) = w.actorat(x, y) {
                if id == w.player {
                    let tile = w.tile(x, y);
                    if (0x49..=0x4C).contains(&tile) && is_facing_tile(x << 16, y << 16, &o) {
                        if w.rnd() % 5 == 0 {
                            // US_RndT, 208c:0ae9
                            items::add_screen_splat(w, 1);
                        }
                        items::take_damage(w, 5, None);
                        super::hud(w);
                    }
                }
                return false;
            }
        }
    }

    // Check for actors in the box grown by one tile.
    if yl > 0 {
        yl -= 1;
    }
    if yh < MAPSIZE as i32 - 1 {
        yh += 1;
    }
    if xl > 0 {
        xl -= 1;
    }
    if xh < MAPSIZE as i32 - 1 {
        xh += 1;
    }
    for y in yl..=yh {
        for x in xl..=xh {
            // `check > objlist`: an object other than objlist[0] (the player).
            let Occ::Obj(c) = w.actorat(x, y) else { continue };
            if c == w.player {
                continue;
            }
            let check = w.obj(c);
            if check.flags & fl::NONMARK != 0 || check.flags2 & fl::NOBLOCK != 0 {
                continue;
            }
            if check.flags & fl::SHOOTABLE == 0 {
                continue;
            }
            let deltax = o.x.wrapping_sub(check.x);
            if !(-MINACTORDIST..=MINACTORDIST).contains(&deltax) {
                continue;
            }
            let deltay = o.y.wrapping_sub(check.y);
            if !(-MINACTORDIST..=MINACTORDIST).contains(&deltay) {
                continue;
            }
            return false;
        }
    }
    true
}

/// *IsFacingTile*(gx, gy, ob) (ghidra 30d6:178d, CONFIRMED): `gx`/`gy` are
/// the tile's top-left corner in 16.16. The tests cascade (they are not
/// exclusive quadrants) and the last one applies at any angle:
/// east-ish and `x < gx`; `angle < 135` and `y > gy`; `angle < 225` and
/// `x > gx`; finally `y < gy`.
#[doc(alias = "IsFacingTile")]
pub(super) fn is_facing_tile(gx: i32, gy: i32, ob: &Obj) -> bool {
    if (ob.angle < 45 || ob.angle > 315) && ob.x < gx {
        return true;
    }
    if ob.angle < 135 && ob.y > gy {
        return true;
    }
    if ob.angle < 225 && ob.x > gx {
        return true;
    }
    ob.y < gy
}
