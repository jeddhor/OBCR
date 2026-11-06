//! Weapons: selection, the magazine/clip model, firing and hitscan
//! (`WOLFSRC/WL_AGENT.C` `CheckWeaponChange` :117, `Cmd_Fire` :983,
//! `GunAttack` :1168, `T_Attack` :1293; OBC segment 208c, docs/re/PLAYER.md
//! §3.8–3.10).
//!
//! Weapons: 0 assault shotgun (unlimited), 1 Uzi, 2 Galil, 3 flame thrower,
//! 4 grenade launcher. `gamestate.ammo` is one shared 30-round magazine;
//! `gamestate.clips[w-1]` counts the clips per weapon, including the loaded one.

use super::{MAGAZINE, MAXCLIPS, hud, snd, update_plu};
use crate::actors;
use crate::data::{AttackStep, S_ATTACK, S_PLAYER};
use crate::events::SimEvent;
use crate::world::{ObjId, World, bt, class, fl};

/// Hard cap on attack steps per call; the real tables end every cycle with
/// code −1 within five steps, so this only trips on corrupt data.
const MAX_ATTACK_STEPS: u32 = 64;

/// `attackinfo[weapon][frame]` (DS:0x0F66), or a terminating step with a
/// fault when out of range (the original would read neighbouring memory).
fn attack_step(w: &mut World, weapon: i16, frame: i16) -> AttackStep {
    let step = usize::try_from(weapon)
        .ok()
        .and_then(|wp| w.data.attackinfo.get(wp))
        .and_then(|row| usize::try_from(frame).ok().and_then(|f| row.get(f)))
        .copied();
    match step {
        Some(s) => s,
        None => {
            w.emit(SimEvent::Fault(format!("attackinfo[{weapon}][{frame}] out of range")));
            AttackStep { tics: 0, attack: -1, frame: 0 }
        }
    }
}

/// Bit test on the owned-weapons byte (`weapons` is read with `cbw`).
fn owns(w: &World, weapon: i16) -> bool {
    let bits = i16::from(w.gamestate.weapons as i8);
    let mask = 1i16.checked_shl(weapon as u32).unwrap_or(0);
    bits & mask != 0
}

/// *HasAmmo*(w) (ghidra 208c:184c, CONFIRMED): the shared magazine is not
/// empty, or the weapon is the shotgun.
#[doc(alias = "HasAmmo")]
fn has_ammo(w: &World, weapon: i16) -> bool {
    w.gamestate.ammo != 0 || weapon == 0
}

/// *HasAmmoOrRedraw*(w) (ghidra 208c:1865, CONFIRMED): [`has_ammo`] that also
/// redraws the ammo bar when it fails.
fn has_ammo_or_redraw(w: &mut World, weapon: i16) -> bool {
    if has_ammo(w, weapon) {
        return true;
    }
    hud(w);
    false
}

/// *LoadMagazine*(w) (ghidra 208c:01ca, CONFIRMED): for weapons 1–4 the
/// magazine becomes 30 if that weapon has any clip, else 0. No clip is used
/// (quirk: re-selecting a weapon is a free reload).
#[doc(alias = "LoadMagazine")]
fn load_magazine(w: &mut World, weapon: i16) {
    if let Some(i) = clip_index(weapon) {
        w.gamestate.ammo = if w.gamestate.clips[i] > 0 { MAGAZINE } else { 0 };
    }
}

/// `clips[]` index for weapons 1–4.
fn clip_index(weapon: i16) -> Option<usize> {
    match weapon {
        1..=4 => Some((weapon - 1) as usize),
        _ => None,
    }
}

/// *CheckWeaponChange* (ghidra 208c:0164, CONFIRMED; `WOLFSRC/WL_AGENT.C:117`):
/// the first owned weapon 0..=bestweapon whose key is down is loaded with
/// *LoadMagazine* and selected if it then has ammo. Otherwise the weapon
/// picture and ammo bar are redrawn (every frame; only a state change emits
/// a HUD event here).
#[doc(alias = "CheckWeaponChange")]
pub(super) fn check_weapon_change(w: &mut World) {
    let mut touched = false;
    let mut i: i16 = 0;
    while i <= w.gamestate.bestweapon {
        let key = usize::try_from(i).ok().and_then(|k| w.buttonstate.get(bt::WEAPON1 + k));
        if owns(w, i) && key.copied().unwrap_or(false) {
            load_magazine(w, i);
            touched = true;
            if has_ammo(w, i) {
                w.gamestate.chosenweapon = i;
                w.gamestate.weapon = i;
                return;
            }
        }
        i += 1;
    }
    if touched {
        hud(w);
    }
}

/// *GiveWeapon*(w) (ghidra 208c:066c, CONFIRMED; `WOLFSRC/WL_AGENT.C:581`):
/// a weapon at least as good as the best one is selected at once; the weapon
/// is marked owned and one clip given.
#[doc(alias = "GiveWeapon")]
pub fn give_weapon(w: &mut World, weapon: i16) {
    // 358f:1969: a dropped weapon of this kind was picked up.
    if w.gamestate.difficulty >= 4 {
        crate::net::clear_drop(w, weapon.clamp(0, 4) as usize);
    }
    if weapon >= w.gamestate.bestweapon {
        w.gamestate.chosenweapon = weapon;
        w.gamestate.bestweapon = weapon;
        w.gamestate.weapon = weapon;
    }
    let bit = 1u8.checked_shl(weapon as u32).unwrap_or(0);
    w.gamestate.weapons |= bit;
    give_clip(w, weapon);
}

/// *GiveClip*(w) (`GiveAmmo`, ghidra 208c:06ab, CONFIRMED; `WOLFSRC/WL_AGENT.C:617`):
/// `clips[w-1] += 1` (cap 19) **and the loaded magazine refills to 30
/// whatever weapon is out** (quirk). The argument is a weapon number.
#[doc(alias = "GiveClip")]
#[doc(alias = "GiveAmmo")]
pub fn give_clip(w: &mut World, weapon: i16) {
    // 358f:1969 for weapon 1 (the Uzi's 9mm clip) in network games.
    if weapon == 1 && w.gamestate.difficulty >= 4 {
        crate::net::clear_drop(w, 1);
    }
    if let Some(i) = clip_index(weapon) {
        let c = &mut w.gamestate.clips[i];
        *c = c.wrapping_add(1);
        w.gamestate.ammo = MAGAZINE;
        if *c >= MAXCLIPS {
            *c = MAXCLIPS;
        }
    }
    hud(w);
}

/// *Reload* (ghidra 208c:1d24, CONFIRMED): uses up the current clip; with
/// clips left the magazine refills, otherwise the weapon falls back to the
/// shotgun, or to the Uzi when 9 mm clips remain (weapons 2–4 only; the Uzi
/// keeps an empty magazine then).
#[doc(alias = "Reload")]
fn reload(w: &mut World) {
    let weapon = w.gamestate.weapon;
    if let Some(i) = clip_index(weapon) {
        let gs = &mut w.gamestate;
        gs.clips[i] = gs.clips[i].wrapping_sub(1);
        if gs.clips[i] > 0 {
            gs.ammo = MAGAZINE;
        } else {
            gs.clips[i] = 0;
            gs.chosenweapon = 0;
            gs.weapon = 0;
            if weapon != 1 && gs.clips[0] > 0 {
                gs.chosenweapon = 1;
                gs.weapon = 1;
            }
        }
    }
    hud(w);
}

/// *UseAmmo* (ghidra 208c:1b72, CONFIRMED): rounds per attack step — Uzi 1,
/// Galil 2, flame thrower 1, grenade launcher 8 — then *Reload* when the
/// magazine is empty. It runs after every step, code-0 steps included.
#[doc(alias = "UseAmmo")]
fn use_ammo(w: &mut World) {
    let cost = match w.gamestate.weapon {
        1 | 3 => 1,
        2 => 2,
        4 => 8,
        _ => return,
    };
    w.gamestate.ammo = w.gamestate.ammo.wrapping_sub(cost);
    if w.gamestate.ammo <= 0 {
        w.gamestate.ammo = 0;
        reload(w);
    }
}

/// *PlayWeaponSound* (ghidra 208c:1883, CONFIRMED): sound `45 + weapon`.
#[doc(alias = "PlayWeaponSound")]
fn play_weapon_sound(w: &mut World) {
    if let Ok(wp) = u16::try_from(w.gamestate.weapon)
        && wp <= 4
    {
        w.play_sound(snd::WEAPON0 + wp);
    }
}

/// `Cmd_Fire` (ghidra 208c:0ed2, CONFIRMED; `WOLFSRC/WL_AGENT.C:983`).
///
/// Deltas: an empty magazine reloads at once; with weapon 0 the breakable
/// wall in front (0x39–0x3C, 0x75–0x78) starts the wall-break animation.
#[doc(alias = "Cmd_Fire")]
pub(super) fn cmd_fire(w: &mut World) {
    w.buttonheld[bt::ATTACK] = true;
    let p = w.player;
    w.obj_mut(p).state = S_ATTACK;
    w.gamestate.weaponframe = 0;
    w.gamestate.attackframe = 0;
    let step = attack_step(w, w.gamestate.weapon, 0);
    w.gamestate.attackcount = i16::from(step.tics);
    w.gamestate.weaponframe = i16::from(step.frame);
    if w.gamestate.ammo <= 0 {
        hud(w);
        reload(w);
    }
    let (x, y, _) = super::interact::facing_tile(w.plr());
    if w.gamestate.weapon != 0 {
        return;
    }
    let count = match w.tile(x, y) {
        0x39 | 0x75 => 6,
        0x3A | 0x76 => 4,
        0x3B | 0x77 => 2,
        0x3C | 0x78 => 1,
        _ => return,
    };
    start_wall_hit(w, x, y);
    w.ply.wallhit_count = count;
}

/// *StartWallHit*(x, y) (ghidra 208c:1035, CONFIRMED; misnamed
/// `SD_PositionSound` by the matcher): the animation runs in
/// [`super::post_actors`].
fn start_wall_hit(w: &mut World, x: i32, y: i32) {
    w.ply.wallhit_active = true;
    w.ply.wallhit_x = x as i16;
    w.ply.wallhit_y = y as i16;
}

/// *StartWallHitAt*(x, y) (ghidra 208c:1c6a, CONFIRMED tables 208c:1cfc,
/// 1d0e, 1d1a), called by the renderer path `1b4a:1494`: starts the
/// wall-break animation on any tile, with a countdown chosen by the tile
/// (6, 4, 2, 1, or unchanged for unlisted tiles).
#[doc(alias = "StartWallHitAt")]
pub fn start_wall_hit_at(w: &mut World, x: i32, y: i32) {
    start_wall_hit(w, x, y);
    let count = match w.tile(x, y) {
        0x39 | 0x75 => 6,
        0x3A | 0x76 => 4,
        0x13 | 0x14 | 0x16 | 0x17 | 0x35 | 0x36 | 0x3B | 0x77 | 0x79 | 0x7A | 0x7C | 0x7D => 2,
        0x3C | 0x78 => 1,
        _ => return,
    };
    w.ply.wallhit_count = count;
}

/// `T_Attack` (ghidra 208c:18dc, CONFIRMED; `WOLFSRC/WL_AGENT.C:1293`).
///
/// The current `attackinfo` entry runs when `attackcount` drops to 0; then
/// `attackcount += tics`, `attackframe++`, `weaponframe = next.frame`.
/// Codes: −1 end (auto re-fire: fire button state is cleared), 0 nothing,
/// 1 shot with ammo check, 2 shot without, 3 loop back while fire is held,
/// 4 loop back while fire is held then shoot, 5 sound only. *UseAmmo* runs
/// after every step but −1. Muzzle light bit 2 is cleared on odd frames.
#[doc(alias = "T_Attack")]
pub fn t_attack(w: &mut World, id: ObjId) {
    if w.buttonstate[bt::USE] && !w.buttonheld[bt::USE] {
        w.buttonstate[bt::USE] = false;
    }
    if w.buttonstate[bt::ATTACK] && !w.buttonheld[bt::ATTACK] {
        w.buttonstate[bt::ATTACK] = false;
    }
    super::movement::control_movement(w, id);
    update_plu(w);
    w.gamestate.attackcount = w.gamestate.attackcount.wrapping_sub(w.tics);

    let mut steps = 0;
    while w.gamestate.attackcount <= 0 {
        steps += 1;
        if steps > MAX_ATTACK_STEPS {
            w.emit(SimEvent::Fault("T_Attack: attack table does not terminate".into()));
            return;
        }
        let cur = attack_step(w, w.gamestate.weapon, w.gamestate.attackframe);
        match cur.attack {
            -1 => {
                end_attack(w, id);
                return;
            }
            1 => attack_shot(w, id),
            2 => {
                if w.gamestate.weapon == 0 {
                    play_weapon_sound(w);
                }
                gun_attack(w, id);
                if w.gamestate.weapon == 0 {
                    play_weapon_sound(w);
                }
                w.ply.lightbits |= 4;
            }
            3 => {
                let weapon = w.gamestate.weapon;
                if has_ammo_or_redraw(w, weapon) && has_ammo(w, weapon) && w.buttonstate[bt::ATTACK]
                {
                    if weapon == 2 || weapon == 1 {
                        w.gamestate.attackframe = -1;
                    } else {
                        w.gamestate.attackframe = w.gamestate.attackframe.wrapping_sub(2);
                    }
                }
                if w.gamestate.weapon == 4 {
                    w.gamestate.ammo = 0; // forces a reload in UseAmmo
                }
                w.ply.lightbits |= 4;
            }
            4 => {
                let weapon = w.gamestate.weapon;
                if has_ammo(w, weapon) && has_ammo_or_redraw(w, weapon) {
                    if weapon == 3 && w.buttonstate[bt::ATTACK] {
                        w.gamestate.attackframe = w.gamestate.attackframe.wrapping_sub(1);
                    } else if w.buttonstate[bt::ATTACK] {
                        w.gamestate.attackframe = w.gamestate.attackframe.wrapping_sub(2);
                    }
                    attack_shot(w, id);
                }
            }
            5 => play_weapon_sound(w),
            _ => {} // 0 and unknown codes
        }
        use_ammo(w);
        let gs = &mut w.gamestate;
        gs.attackcount = gs.attackcount.wrapping_add(i16::from(cur.tics));
        gs.attackframe = gs.attackframe.wrapping_add(1);
        let next = attack_step(w, w.gamestate.weapon, w.gamestate.attackframe);
        w.gamestate.weaponframe = i16::from(next.frame);
        if w.gamestate.attackframe & 1 == 1 {
            w.ply.lightbits &= !4;
        }
    }
}

/// Attack code 1 (208c:1a54): with ammo, the weapon sound (not for the flame
/// thrower), *GunAttack*, shot and hit counters, muzzle light; without ammo
/// the next entry is skipped.
fn attack_shot(w: &mut World, id: ObjId) {
    let weapon = w.gamestate.weapon;
    if !(has_ammo(w, weapon) && has_ammo_or_redraw(w, weapon)) {
        w.gamestate.attackframe = w.gamestate.attackframe.wrapping_add(1);
        return;
    }
    if weapon != 3 {
        play_weapon_sound(w);
    }
    gun_attack(w, id);
    w.gamestate.shots_level = w.gamestate.shots_level.wrapping_add(1);
    w.gamestate.hits_level = w.gamestate.hits_level.wrapping_add(i16::from(w.ply.gunhit));
    w.ply.lightbits |= 4;
}

/// Attack code −1 (208c:1987): back to `s_player`; with an empty magazine
/// step down to the next owned weapon that has ammo (in practice the
/// shotgun, since the magazine is shared); otherwise return to the chosen
/// weapon. Clears `buttonstate`/`buttonheld` of fire, so holding fire
/// starts the next cycle on the next frame.
fn end_attack(w: &mut World, id: ObjId) {
    w.obj_mut(id).state = S_PLAYER;
    let weapon = w.gamestate.weapon;
    if !has_ammo(w, weapon) {
        while w.gamestate.weapon > 0 {
            w.gamestate.weapon -= 1;
            let wp = w.gamestate.weapon;
            if owns(w, wp) && has_ammo_or_redraw(w, wp) {
                break;
            }
        }
        hud(w);
    } else {
        let chosen = w.gamestate.chosenweapon;
        if weapon != chosen && has_ammo(w, chosen) && has_ammo_or_redraw(w, chosen) {
            w.gamestate.weapon = chosen;
        }
    }
    w.gamestate.weaponframe = 0;
    w.gamestate.attackframe = 0;
    w.buttonheld[bt::ATTACK] = false;
    w.buttonstate[bt::ATTACK] = false;
}

/// `GunAttack(ob)` (ghidra 208c:14e0, CONFIRMED; `WOLFSRC/WL_AGENT.C:1168`).
///
/// Weapon 4 launches a grenade and weapon 3 a flame (no target). Weapons
/// 0–2 are hitscan: the nearest (`transx`) shootable, visible object within
/// `shootdelta` of `centerx`, retried while `CheckLine` fails (Wolf3D's retry
/// only accepts nearer objects). Damage by tile distance plus `US_RndT() % 50`
/// and a weapon bonus, then the difficulty multiplier (0 → ×8, 1 and 3 → ×2).
/// Some classes may splat the visor first. No long-range miss roll.
#[doc(alias = "GunAttack")]
pub(super) fn gun_attack(w: &mut World, ob: ObjId) {
    w.ply.gunhit = false;
    w.madenoise = true;
    let mut viewdist = i32::MAX;
    let mut closest: Option<ObjId> = None;

    let network = w.gamestate.difficulty >= 4;
    if w.gamestate.weapon == 4 || w.gamestate.weapon == 3 {
        // Network: report the shot (358f:13a2) and stamp our id (358f:1995).
        if network {
            crate::net::report_fire(w, -1);
        }
        let before = w.lastobj;
        if w.gamestate.weapon == 4 {
            actors::spawn_grenade(w, None);
        } else {
            actors::spawn_player_flame(w, None);
        }
        if network && let Some(n) = w.lastobj.filter(|&n| Some(n) != before) {
            w.obj_mut(n).temp2 = crate::net::my_id(w) as i16;
        }
        return;
    }

    let target = loop {
        let oldclosest = closest;
        let mut cur = w.obj(ob).next;
        let mut guard = 0;
        while let Some(c) = cur {
            guard += 1;
            if guard > crate::world::MAXACTORS {
                break;
            }
            let check = w.obj(c);
            if check.flags & fl::SHOOTABLE != 0
                && check.flags & fl::VISABLE != 0
                && check.viewx.wrapping_sub(w.ply.centerx).wrapping_abs() < w.ply.shootdelta
                && check.transx < viewdist
            {
                viewdist = check.transx;
                closest = Some(c);
            }
            cur = check.next;
        }
        if closest == oldclosest {
            // No more targets: all missed (network: report a miss, 358f:13a2).
            if network {
                crate::net::report_fire(w, -1);
            }
            return;
        }
        let Some(c) = closest else { return };
        if actors::check_line(w, c) {
            break c;
        }
    };

    // Network: report the hit (358f:13a2); the victim's machine decides the
    // damage, here the puppet only shows pain (358f:135a).
    if network {
        let id = w.obj(target).temp2;
        crate::net::report_fire(w, id);
        crate::net::hit_puppet(w, target);
        return;
    }
    w.ply.gunhit = true;
    let t = *w.obj(target);
    let p = *w.plr();
    let dx = (t.tilex as i16).wrapping_sub(p.tilex as i16).wrapping_abs();
    let dy = (t.tiley as i16).wrapping_sub(p.tiley as i16).wrapping_abs();
    let dist = dx.max(dy);

    // Visor splats from some classes (table 208c:1822).
    let chance = match t.obclass {
        class::SOLDIER | class::BLACK_SUIT => Some((35, true)),
        class::GIANT_RAT => Some((10, true)),
        class::GUNNER | class::RED_VEST | class::RED_GIANT => Some((25, true)),
        class::FAT_MAN => Some((15, true)),
        class::EXPLOSION => Some((5, false)),
        _ => None,
    };
    if let Some((n, near_only)) = chance
        && (!near_only || dist <= 2)
        && w.rnd() % n == 0
    // US_RndT, 208c:1694 … 1734
    {
        super::items::add_screen_splat(w, 2);
    }

    let r = w.rnd(); // US_RndT, 208c:1755
    let damage = hit_damage(dist, r, w.gamestate.weapon, w.gamestate.difficulty);
    actors::damage_actor(w, target, damage);
}

/// The hitscan damage of `GunAttack` (208c:1755–1812) for tile distance
/// `dist` and the raw `US_RndT()` value `rnd`.
fn hit_damage(dist: i16, rnd: i16, weapon: i16, difficulty: i16) -> i16 {
    let mut r = rnd % 50;
    r += match weapon {
        0 => 50,
        1 => 20,
        2 => 50,
        _ => 0,
    };
    let damage = if dist < 1 {
        r + 100
    } else if dist < 2 {
        r + 75
    } else if dist < 4 {
        r + 50
    } else if dist < 6 {
        r + 25
    } else if dist < 8 {
        r + 10
    } else if dist < 10 {
        r + 5
    } else {
        20
    };
    match difficulty {
        0 => damage.wrapping_shl(3),
        1 | 3 => damage.wrapping_shl(1),
        _ => damage,
    }
}

#[cfg(test)]
mod tests {
    use super::hit_damage;

    #[test]
    fn damage_table() {
        // dist 0, rnd 0, shotgun, difficulty 2 (×1): 0 + 50 + 100.
        assert_eq!(hit_damage(0, 0, 0, 2), 150);
        // rnd % 50 and the Uzi bonus 20 at dist 1: 49 + 20 + 75.
        assert_eq!(hit_damage(1, 99, 1, 2), 144);
        assert_eq!(hit_damage(3, 0, 2, 2), 100);
        assert_eq!(hit_damage(5, 0, 2, 2), 75);
        assert_eq!(hit_damage(7, 0, 2, 2), 60);
        assert_eq!(hit_damage(9, 0, 2, 2), 55);
        // Long range: a flat 20, no random part, no weapon bonus.
        assert_eq!(hit_damage(10, 255, 2, 2), 20);
        // Difficulty multipliers: 0 → ×8, 1 and 3 → ×2, others ×1.
        assert_eq!(hit_damage(10, 0, 0, 0), 160);
        assert_eq!(hit_damage(10, 0, 0, 1), 40);
        assert_eq!(hit_damage(10, 0, 0, 3), 40);
        assert_eq!(hit_damage(10, 0, 0, 5), 20);
    }
}
