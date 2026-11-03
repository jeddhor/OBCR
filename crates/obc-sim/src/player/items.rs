//! Pickups, damage, healing, score and the visor effects (`WOLFSRC/WL_AGENT.C`
//! `TakeDamage` :386, `HealSelf` :434, `GivePoints` :523, `GetBonus` :667;
//! `WL_PLAY.C` palette flashes; OBC docs/re/PLAYER.md §4, §5.4).

use super::weapons::{give_clip, give_weapon};
use super::{MAGAZINE, MAXCLIPS, MAXSPLATS, MAXVALUE, Splat, hud, message, snd};
use crate::actors;
use crate::events::SimEvent;
use crate::world::{ObjId, World, ex};

/// Score cap (0xF4240) in *GivePoints*.
const MAXSCORE: i32 = 1_000_000;

/// `TakeDamage(points, attacker)` (ghidra 208c:0494, CONFIRMED;
/// `WOLFSRC/WL_AGENT.C:386`).
///
/// OBC: while health ≥ 30 the points are scaled by `difficulty/2 + 1`; the
/// damage flash is a fixed 6; armor absorbs half (rounded down) and loses
/// that half, health takes the rest scaled again by difficulty (0, 1 → ×2,
/// 2 → ×4, 3 → ×3); at health ≤ 35 a 1/15 roll fades to black (single
/// player); at 0 the game is over (`ex_died`). God mode and the immunity
/// flag (`gamestate+0x28`) skip the damage but not the flash.
#[doc(alias = "TakeDamage")]
pub fn take_damage(w: &mut World, points: i16, attacker: Option<ObjId>) {
    let mut points = points;
    if w.gamestate.difficulty >= 4 {
        // TODO(M13): network filter: team mode 5 ignores friendly attackers
        // (358f:199d); only other players and classes 0x12/0x13 count (358f:1259).
    }
    if w.gamestate.health >= 30 {
        points = points.wrapping_mul(w.gamestate.difficulty / 2 + 1);
    }
    // DS:0xBFF4 (a modal box is up) returns here; there is no modal box in the simulation.
    w.lastattacker = attacker;
    start_damage_flash(w, 6);

    if !w.godmode && w.gamestate.immune == 0 {
        if w.gamestate.armor > 0 {
            points >>= 1;
            w.gamestate.armor = w.gamestate.armor.wrapping_sub(points);
            if w.gamestate.armor < 0 {
                w.gamestate.armor = 0;
            }
            hud(w);
        }
        points = match w.gamestate.difficulty {
            0 | 1 => points.wrapping_mul(2),
            2 => points.wrapping_mul(4),
            3 => points.wrapping_mul(3),
            _ => points,
        };
        let gs = &mut w.gamestate;
        gs.health = gs.health.wrapping_sub(points).clamp(0, MAXVALUE);
        hud(w);
        // Enhanced-mode hint only (not in the original): gamepad rumble.
        w.emit(SimEvent::Rumble(points.clamp(0, 100) as u8));
    }

    if !w.godmode && w.gamestate.health <= 35 {
        let r = w.rnd(); // US_RndT, 208c:0592
        if r % 15 == 0 && w.gamestate.difficulty < 4 {
            // VL_FadeOut(0, 255, 0, 0, 0, 50)
            w.emit(SimEvent::Blackout);
        }
    }

    if w.gamestate.health <= 0 {
        // TODO(M13): difficulty >= 4 notifies the network (358f:182c).
        w.gamestate.health = 0;
        w.playstate = ex::DIED;
        w.killerobj = attacker;
    }
}

/// `HealSelf(points)` (ghidra 208c:05f4, CONFIRMED; `WOLFSRC/WL_AGENT.C:434`):
/// health += points, then **clamped down to `maxhealth`** (100 on most
/// levels), so an unguarded call can lower health.
#[doc(alias = "HealSelf")]
pub fn heal_self(w: &mut World, points: i16) {
    let gs = &mut w.gamestate;
    gs.health = gs.health.wrapping_add(points);
    if gs.health > gs.maxhealth {
        gs.health = gs.maxhealth;
    }
    hud(w);
}

/// `GivePoints(points)` (ghidra 208c:0614, CONFIRMED; `WOLFSRC/WL_AGENT.C:523`):
/// score capped at 1,000,000; no extra lives; nothing in network games.
///
/// TODO(M9): the original also skips points while DS:0x2BC8 is set (an ally
/// is dealing the damage); that flag belongs to `allies.rs`.
#[doc(alias = "GivePoints")]
pub fn give_points(w: &mut World, points: i32) {
    if w.gamestate.difficulty >= 4 {
        return;
    }
    let gs = &mut w.gamestate;
    gs.score = gs.score.wrapping_add(points);
    if gs.score > MAXSCORE {
        gs.score = MAXSCORE;
    }
    hud(w);
}

/// `GetBonus(check)` (ghidra 208c:074f, CONFIRMED; `WOLFSRC/WL_AGENT.C:667`)
/// for static `index`. Items, by the static's item byte:
///
/// | Item | Effect |
/// |---|---|
/// | 3 | supply crate (*GiveSupplyCrate*), maxammo += 30 |
/// | 4 | grenades, if fewer than 19 |
/// | 5 | Galil (weapon 2) and an extra clip |
/// | 8 | flak jacket: armor += 100 if below 400 |
/// | 9 | increased health: +100 if below 400, and max health = health |
/// | 10 | Map ThermoSensor |
/// | 11 | 9 mm clip (Uzi), if fewer than 19 |
/// | 12, 13 | flame thrower, grenade launcher |
/// | 14 | fuel pack, if fewer than 19 fuel clips |
/// | 20 | booby trap: blast ahead, `TakeDamage(US_RndT())`, knock-back |
///
/// A taken item gets `shape = -1`.
#[doc(alias = "GetBonus")]
pub fn get_bonus(w: &mut World, index: usize) {
    let Some(s) = w.statics.get(index).copied() else { return };
    let gs = &w.gamestate;
    let picked = match u16::from(s.item as u8).wrapping_sub(3) {
        // 20: booby trap (208c:0775)
        17 => {
            w.emit(SimEvent::Shake(4)); // DS:0x0D40 = 4
            actors::spawn_explosion_ahead(w);
            let r = w.rnd(); // US_RndT, 208c:0781
            take_damage(w, r, None);
            w.play_sound(snd::BOOBY_TRAP);
            message(w, "BOOBY TRAP");
            w.ply.knockback_trigger = 10;
            true
        }
        // 8: flak jacket (208c:07b2)
        5 if gs.armor < 400 => {
            let gs = &mut w.gamestate;
            gs.armor = gs.armor.wrapping_add(100);
            if gs.armor > MAXVALUE {
                gs.armor = MAXVALUE;
            }
            message(w, "Flak Jacket");
            w.emit(SimEvent::ItemIcon(8));
            hud(w);
            true
        }
        // 9: increased health (208c:07f6); health itself is capped per frame
        6 if gs.health < 400 => {
            let gs = &mut w.gamestate;
            gs.health = gs.health.wrapping_add(100);
            gs.maxhealth = gs.health;
            if gs.maxhealth > MAXVALUE {
                gs.maxhealth = MAXVALUE;
            }
            message(w, "Increased Health");
            w.emit(SimEvent::ItemIcon(9));
            hud(w);
            true
        }
        // 3: supply crate (208c:0840)
        0 => {
            let gs = &mut w.gamestate;
            gs.maxammo = gs.maxammo.wrapping_add(30);
            if gs.maxammo > MAXVALUE {
                gs.maxammo = MAXVALUE;
            }
            give_supply_crate(w);
            w.emit(SimEvent::ItemIcon(3));
            hud(w);
            true
        }
        // 4: grenades (208c:0870)
        1 if gs.clips[3] < MAXCLIPS => {
            give_clip(w, 4);
            message(w, "Grenades!");
            w.emit(SimEvent::ItemIcon(4));
            hud(w);
            true
        }
        // 5: Galil (208c:08a9): GiveWeapon already gives one clip, then another.
        2 => {
            give_weapon(w, 2);
            give_clip(w, 2);
            w.play_sound(snd::ITEM);
            message(w, "Galil");
            true
        }
        // 11: Uzi clip (208c:08d9)
        8 if gs.clips[0] < MAXCLIPS || gs.difficulty >= 4 => {
            give_clip(w, 1);
            w.play_sound(snd::ITEM);
            message(w, "Uzi");
            true
        }
        // 12: flame thrower (208c:0914)
        9 => {
            give_weapon(w, 3);
            give_clip(w, 3);
            message(w, "Flame Thrower");
            true
        }
        // 13: grenade launcher (208c:093a)
        10 => {
            give_weapon(w, 4);
            give_clip(w, 4);
            message(w, "Grenade Launcher");
            true
        }
        // 10: Map ThermoSensor (208c:095f)
        7 => {
            message(w, "Map ThermoSensor");
            w.thermosensor = true;
            true
        }
        // 14: fuel pack (208c:0978)
        11 if gs.clips[2] < MAXCLIPS => {
            let gs = &mut w.gamestate;
            gs.fuel_cap = gs.fuel_cap.wrapping_add(100);
            if gs.fuel_cap > MAXVALUE {
                gs.fuel_cap = MAXVALUE;
            }
            message(w, "Fuel Pack");
            w.emit(SimEvent::ItemIcon(14));
            give_clip(w, 3);
            hud(w);
            true
        }
        _ => false,
    };
    if picked && let Some(st) = w.statics.get_mut(index) {
        st.shape = -1;
    }
}

/// *GiveSupplyCrate* (ghidra 208c:1e18, CONFIRMED): `r = US_RndT() + 1`;
/// 1–5 a fuel clip (message and refill only when that reaches the cap,
/// probably an inverted test), 6–20 one to four grenades, 21–64 one to seven
/// 7 mm clips, else one to seven 9 mm clips (caps 19).
#[doc(alias = "GiveSupplyCrate")]
fn give_supply_crate(w: &mut World) {
    let r = w.rnd() + 1; // US_RndT, 208c:1e1c
    if (1..=5).contains(&r) {
        let c = &mut w.gamestate.clips[2];
        *c = c.wrapping_add(1);
        if *c >= MAXCLIPS {
            *c = MAXCLIPS;
            w.gamestate.ammo = MAGAZINE;
            message(w, "Fuel Bottle");
        }
        return;
    }
    if (6..=20).contains(&r) {
        let n = w.rnd() / 64 + 1; // US_RndT, 208c:1e63
        let c = &mut w.gamestate.clips[3];
        *c = c.wrapping_add(n);
        if *c >= MAXCLIPS {
            *c = MAXCLIPS;
            w.gamestate.ammo = MAGAZINE;
        }
        message(w, if n > 1 { format!(" {n} Grenades") } else { format!(" {n} Grenade") });
        return;
    }
    let (clip, plural, single) = if (21..=64).contains(&r) {
        (1, "7mm Clips", "7mm Clip")
    } else {
        (0, "9mm Clips", "9mm Clip")
    };
    let n = w.rnd() / 37 + 1; // US_RndT, 208c:1eb8 / 1f07
    let c = &mut w.gamestate.clips[clip];
    *c = c.wrapping_add(n);
    if *c >= MAXCLIPS {
        *c = MAXCLIPS;
    }
    w.play_sound(snd::ITEM);
    message(w, if n > 1 { format!(" {n} {plural}") } else { format!(" {n} {single}") });
}

/// *StartDamageFlash*(damage) (ghidra 1942:0f9a, CONFIRMED; `WOLFSRC/WL_PLAY.C:1157`).
#[doc(alias = "StartDamageFlash")]
pub fn start_damage_flash(w: &mut World, damage: i16) {
    w.emit(SimEvent::DamageFlash(damage));
}

/// *StartBonusFlash* (ghidra 1942:0f44, CONFIRMED; `WOLFSRC/WL_PLAY.C:1143`):
/// in single player, 1 in 15 bonus flashes is a red flash of
/// `US_RndT() % 100` instead. Nothing in network games.
#[doc(alias = "StartBonusFlash")]
pub fn start_bonus_flash(w: &mut World) {
    if w.gamestate.difficulty >= 4 {
        return;
    }
    if w.rnd() % 15 == 0 {
        // US_RndT, 1942:0f4e
        let r = w.rnd() % 100; // US_RndT, 1942:0f5d
        start_damage_flash(w, r);
        return;
    }
    w.emit(SimEvent::BonusFlash);
}

/// *AddScreenSplat*(kind) (ghidra 30d6:0fbe, CONFIRMED by instruction read):
/// up to 19 splats; sprite `US_RndT() % 8`, x `US_RndT()` clamped to
/// 64..viewwidth-64, y `US_RndT()` clamped to viewheight, type `kind % 3`;
/// type 1 also plays splat sound 62 or 63 (one more `US_RndT`).
#[doc(alias = "AddScreenSplat")]
pub fn add_screen_splat(w: &mut World, kind: i16) {
    if usize::from(w.ply.splatcount) >= MAXSPLATS {
        return;
    }
    let shape = w.rnd() % 8; // US_RndT, 30d6:0fcf
    let mut x = w.rnd(); // US_RndT, 30d6:0fe8
    if x < 0x40 {
        x = 0x40;
    }
    let right = w.ply.viewwidth.wrapping_sub(0x40);
    if x > right {
        x = right;
    }
    let mut y = w.rnd(); // US_RndT, 30d6:1049
    if y > w.ply.viewheight {
        y = w.ply.viewheight;
    }
    let kind = kind % 3;
    let n = usize::from(w.ply.splatcount);
    w.ply.splats[n] = Splat { shape, x, y, kind };
    w.ply.splatcount += 1;
    w.emit(SimEvent::Splat(kind as u8));
    if kind == 1 {
        let r = (w.rnd() % 2) as usize; // US_RndT, 30d6:10a4
        w.play_sound(snd::SPLAT[r]);
    }
}

/// *ClearScreenSplats* (ghidra 30d6:10c5, CONFIRMED): wipes the visor
/// (W key, ally switch, PlayLoop start).
#[doc(alias = "ClearScreenSplats")]
pub fn clear_screen_splats(w: &mut World) {
    w.ply.splatcount = 0;
}
