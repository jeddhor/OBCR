//! The use key: push walls, doors, switches, the medic wall, generators and
//! exits (`Cmd_Use`, `WOLFSRC/WL_AGENT.C:1008`; OBC ghidra 208c:104c).

use super::{heal_self, message, snd};
use crate::events::SimEvent;
use crate::world::{Obj, World, bt, ex};

/// Plane-1 push-wall marker (98).
const PUSHABLETILE: u16 = 0x62;

/// The tile in front of `ob` by angle quadrant, as `Cmd_Use` and `Cmd_Fire`
/// pick it (east if angle < 45 or > 315, north < 135, west < 225, else
/// south), plus the `PushWall` direction (0 N, 1 E, 2 S, 3 W).
pub(super) fn facing_tile(ob: &Obj) -> (i32, i32, u16) {
    let tx = i32::from(ob.tilex as i16);
    let ty = i32::from(ob.tiley as i16);
    if ob.angle < 45 || ob.angle > 315 {
        (tx + 1, ty, 1)
    } else if ob.angle < 135 {
        (tx, ty - 1, 0)
    } else if ob.angle < 225 {
        (tx - 1, ty, 3)
    } else {
        (tx, ty + 1, 2)
    }
}

/// `Cmd_Use` (ghidra 208c:104c, CONFIRMED; `WOLFSRC/WL_AGENT.C:1008`).
///
/// Order: plane-1 push-wall marker (no edge test), then the `buttonheld`
/// edge test, then doors (`tilemap & 0x80`, no key check), then the tile
/// switch: exits 0x71–0x73, medic wall 0x58, generator switch 0x5F/0x60,
/// special panel 0x55, door-look wall switches. After a switch or the medic
/// wall, plane 1 there is either an animated-wall code (0x67/0x6A →
/// `OperateAnimWall`) or cleared.
#[doc(alias = "Cmd_Use")]
pub(super) fn cmd_use(w: &mut World) {
    let (x, y, dir) = facing_tile(w.plr());
    let doornum = w.tile(x, y);
    let info = w.mapspot(x, y, 1);

    if info == PUSHABLETILE {
        push_wall(w, x, y, dir);
        return;
    }

    // Edge trigger. The network flag DS:0xEF93 (358f:198d) is 0 in single player.
    if w.buttonheld[bt::USE] {
        return;
    }
    // TODO(M13): difficulty >= 4 also calls 358f:1979 here.
    w.buttonheld[bt::USE] = true;

    if doornum & 0x80 != 0 {
        w.operate_door(usize::from(doornum & !0x80));
        return;
    }

    match doornum {
        0x71 | 0x72 => {
            // Ladder / stairs.
            w.ply.exitkind = 0;
            if w.gamestate.difficulty < 4 {
                try_exit(w);
            } else {
                w.playstate = ex::COMPLETED;
            }
        }
        0x73 => {
            // Elevator: alternate ending sequence F4.
            w.ply.exitkind = 1;
            try_exit(w);
        }
        0x58 => {
            // Medic wall: only while health is below the level's maximum.
            if w.gamestate.health >= w.gamestate.maxhealth {
                return;
            }
            heal_self(w, 25);
            message(w, "Medic Pack");
            switch_info(w, x, y, info);
        }
        0x60 => {
            // Generator switch: on.
            set_tile(w, x, y, 0x5F);
            // FinishPaletteShifts (1942:10c9) is presentation; SetPaletteMode(1).
            w.emit(SimEvent::Lighting(1));
            message(w, "GENERATORS ON");
            w.play_sound(snd::GENERATOR);
        }
        0x5F => {
            set_tile(w, x, y, 0x60);
            w.emit(SimEvent::Lighting(2));
            message(w, "GENERATORS OFF");
            w.play_sound(snd::GENERATOR);
        }
        0x55 => operate_anim_wall(w, x, y),
        0x0D | 0x10 => {
            w.play_sound(snd::SWITCH_A);
            switch_info(w, x, y, info);
        }
        0x1D | 0x20 => {
            w.play_sound(snd::SWITCH_B);
            switch_info(w, x, y, info);
        }
        0x31 | 0x34 => {
            w.play_sound(snd::SWITCH_C);
            switch_info(w, x, y, info);
        }
        0x41 | 0x44 | 0x45 | 0x48 => {
            w.play_sound(snd::SWITCH_D);
            switch_info(w, x, y, info);
        }
        _ => {}
    }
}

/// The plane-1 rule after a switch or the medic wall (208c:12e9).
fn switch_info(w: &mut World, x: i32, y: i32, info: u16) {
    if info == 0x6A || info == 0x67 {
        operate_anim_wall(w, x, y);
    } else {
        w.set_mapspot(x, y, 1, 0);
    }
}

fn set_tile(w: &mut World, x: i32, y: i32, tile: u8) {
    if World::in_map(x, y) {
        let i = World::idx(x, y);
        w.tilemap[i] = tile;
    }
}

/// *TryExit* (ghidra 30d6:1bc0, CONFIRMED): nothing on map 39 (the game ends
/// by killing the boss); otherwise at least half the floor's enemies must be
/// dead, or the message `UNFINISHED BUSINESS` appears.
#[doc(alias = "TryExit")]
pub(super) fn try_exit(w: &mut World) {
    let ok = (w.gamestate.killtotal >> 1) <= w.gamestate.killcount;
    if w.gamestate.mapon == 39 {
        return;
    }
    if ok {
        w.playstate = ex::COMPLETED;
    } else {
        message(w, "UNFINISHED BUSINESS");
    }
}

/// `PushWall(x, y, dir)` (ghidra 2281:0ce2, docs/re/LEVEL_CODES.md §4.7).
///
/// TODO(M5): owned by the push-wall module, which does not exist yet; call
/// its `World::push_wall(x, y, dir)` here once it lands.
#[doc(alias = "PushWall")]
fn push_wall(w: &mut World, x: i32, y: i32, dir: u16) {
    let _ = (w, x, y, dir);
}

/// `OperateAnimWall(x, y)` (ghidra 2f4d:08e8, docs/re/LEVEL_CODES.md §4.4):
/// opens or closes the door-look wall / barrier at the tile.
///
/// TODO(M5): owned by the animated-wall module (list DS:0x46E2), which does
/// not exist yet; call its `World::operate_anim_wall(x, y)` here.
#[doc(alias = "OperateAnimWall")]
fn operate_anim_wall(w: &mut World, x: i32, y: i32) {
    let _ = (w, x, y);
}
