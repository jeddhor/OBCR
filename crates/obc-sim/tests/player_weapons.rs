//! Weapon selection, the magazine/clip model and attack timings
//! (docs/re/PLAYER.md §3.8–3.9).

mod player_support;

use obc_formats::DataDir;
use obc_formats::exe::MzExe;
use obc_sim::data::{GameData, S_ATTACK, S_PLAYER};
use obc_sim::player;
use obc_sim::world::{World, bt};
use player_support::*;

/// Holds fire from frame 1 for `frames` frames (1 tic each); returns the
/// frames (1-based, the press is frame 1) in which `GunAttack` ran.
fn hold_fire(w: &mut World, frames: u32) -> Vec<u32> {
    let fire = with_button(cmd(), bt::ATTACK);
    let mut shots = Vec::new();
    for f in 1..=frames {
        frame(w, &fire);
        if w.madenoise {
            shots.push(f);
        }
    }
    shots
}

fn armed(weapon: i16) -> World {
    let mut w = room_world();
    start(&mut w, 1, 12, 12, 1);
    if weapon >= 2 {
        player::give_weapon(&mut w, weapon);
    }
    w.gamestate.weapon = weapon;
    w.gamestate.chosenweapon = weapon;
    w
}

#[test]
fn shotgun_fires_once_per_81_frames() {
    let mut w = armed(0);
    // Entry 1 (code 2) fires 12 tics after the press; the cycle ends at
    // +80 with code −1, which clears the fire button so the held button
    // re-triggers Cmd_Fire on the next frame (+81).
    let shots = hold_fire(&mut w, 200);
    assert_eq!(shots, [13, 94, 175]);
    assert_eq!(w.gamestate.ammo, 30, "the shotgun uses no ammo");
    // Code 2 plays the shotgun sound before and after the shot.
    let ev = take_events(&mut w);
    assert_eq!(sounds(&ev).iter().filter(|&&s| s == 45).count(), 6);
    assert_eq!(w.gamestate.shots_level, 0, "only code-1 shots are counted");
}

#[test]
fn uzi_fires_three_shots_per_24_tics_using_four_rounds() {
    let mut w = armed(1);
    let shots = hold_fire(&mut w, 121);
    let expect: Vec<u32> = (1..=20u32).filter(|k| (k - 1) % 4 != 3).map(|k| 1 + 6 * k).collect();
    assert_eq!(shots, expect);
    assert_eq!(shots.len(), 15);
    assert_eq!(w.gamestate.ammo, 30 - 20, "UseAmmo runs on every step");
    assert_eq!(w.gamestate.shots_level, 5);
    assert_eq!(w.plr().state, S_ATTACK);
    assert_eq!(sounds(&take_events(&mut w)).iter().filter(|&&s| s == 46).count(), 5);
}

#[test]
fn uzi_releasing_fire_ends_the_cycle() {
    let mut w = armed(1);
    frame(&mut w, &with_button(cmd(), bt::ATTACK));
    for _ in 0..40 {
        frame(&mut w, &cmd());
    }
    assert_eq!(w.plr().state, S_PLAYER);
    assert_eq!(w.gamestate.ammo, 30 - 4, "steps 0–3 use a round each; the -1 step none");
    assert_eq!((w.gamestate.attackframe, w.gamestate.weaponframe), (0, 0));
}

#[test]
fn galil_uses_two_rounds_per_step_and_reloads() {
    let mut w = armed(2);
    w.gamestate.clips[1] = 3;
    w.gamestate.ammo = 30;
    let shots = hold_fire(&mut w, 1 + 6 * 15);
    assert_eq!(shots.len(), 12);
    // Step 15 empties the magazine: Reload takes a clip and refills.
    assert_eq!(w.gamestate.clips[1], 2);
    assert_eq!(w.gamestate.ammo, 30);
    assert_eq!(w.gamestate.weapon, 2);
}

#[test]
fn flame_thrower_fires_every_6_tics_after_18() {
    let mut w = armed(3);
    assert_eq!((w.gamestate.weapon, w.gamestate.clips[2], w.gamestate.ammo), (3, 1, 30));
    let shots = hold_fire(&mut w, 1 + 6 * 12);
    let expect: Vec<u32> = (3..=12u32).map(|k| 1 + 6 * k).collect();
    assert_eq!(shots, expect);
    assert_eq!(w.gamestate.ammo, 30 - 12);
    // Code 5 (entry 1) plays the flame sound; code 1/4 shots do not.
    assert_eq!(sounds(&take_events(&mut w)).iter().filter(|&&s| s == 48).count(), 1);
}

#[test]
fn grenade_launcher_fires_once_per_26_tics_one_clip_each() {
    let mut w = armed(4);
    w.gamestate.clips[3] = 3;
    w.gamestate.ammo = 30;
    let shots = hold_fire(&mut w, 1 + 70);
    assert_eq!(shots, [19, 45, 71]);
    // Two code-3 steps forced a reload each.
    assert_eq!(w.gamestate.clips[3], 1);
    assert_eq!(w.gamestate.ammo, 22);
    assert_eq!(w.gamestate.shots_level, 3);
}

#[test]
fn uzi_runs_dry_and_falls_back_to_the_shotgun() {
    let mut w = armed(1);
    w.gamestate.clips[0] = 1;
    hold_fire(&mut w, 1 + 6 * 30 + 100);
    assert_eq!(w.gamestate.clips[0], 0);
    assert_eq!((w.gamestate.weapon, w.gamestate.chosenweapon), (0, 0));
    assert_eq!(w.gamestate.ammo, 0);
}

#[test]
fn weapon_keys_load_the_magazine_first() {
    let mut w = armed(1);
    // Galil owned but no clips: LoadMagazine empties the magazine and the
    // switch fails (quirk 1).
    w.gamestate.weapons |= 1 << 2;
    w.gamestate.bestweapon = 2;
    w.gamestate.clips[1] = 0;
    frame(&mut w, &with_button(cmd(), bt::WEAPON1 + 2));
    assert_eq!(w.gamestate.weapon, 1);
    assert_eq!(w.gamestate.ammo, 0);

    // Re-selecting the Uzi is a free reload.
    w.gamestate.ammo = 5;
    frame(&mut w, &with_button(cmd(), bt::WEAPON1 + 1));
    assert_eq!((w.gamestate.ammo, w.gamestate.clips[0]), (30, 2));

    // The shotgun is always selectable; weapons above bestweapon are ignored.
    frame(&mut w, &with_button(cmd(), bt::WEAPON1));
    assert_eq!((w.gamestate.weapon, w.gamestate.chosenweapon), (0, 0));
    w.gamestate.weapons |= 1 << 4;
    w.gamestate.clips[3] = 5;
    frame(&mut w, &with_button(cmd(), bt::WEAPON1 + 4));
    assert_eq!(w.gamestate.weapon, 0);
}

#[test]
fn give_clip_and_give_weapon() {
    let mut w = armed(1);
    w.gamestate.ammo = 3;
    player::give_clip(&mut w, 3);
    assert_eq!(w.gamestate.clips[2], 1);
    assert_eq!(w.gamestate.ammo, 30, "any clip refills the loaded magazine");
    w.gamestate.clips[0] = 19;
    player::give_clip(&mut w, 1);
    assert_eq!(w.gamestate.clips[0], 19, "capped at 19");

    // A weapon at least as good as the best one is selected.
    player::give_weapon(&mut w, 2);
    assert_eq!((w.gamestate.weapon, w.gamestate.bestweapon, w.gamestate.chosenweapon), (2, 2, 2));
    assert_eq!(w.gamestate.weapons, 0b111);
    assert_eq!(w.gamestate.clips[1], 1);
    player::give_weapon(&mut w, 1);
    assert_eq!(w.gamestate.weapon, 2, "a worse weapon is not selected");
}

#[test]
fn empty_magazine_reloads_on_fire() {
    let mut w = armed(1);
    w.gamestate.ammo = 0;
    frame(&mut w, &with_button(cmd(), bt::ATTACK));
    assert_eq!((w.gamestate.ammo, w.gamestate.clips[0]), (30, 1));
}

#[test]
fn shotgun_starts_the_glass_break() {
    let mut w = armed(0);
    set_wall(&mut w, 15, 12, 0x39);
    // Move next to the wall so it is the tile in front.
    w.obj_mut(0).x = (14 << 16) + 0x8000;
    w.obj_mut(0).tilex = 14;
    frame(&mut w, &with_button(cmd(), bt::ATTACK));
    // Countdown 6: the tile advances on even counts (6, 4, 2) and ends at 0x3C.
    for _ in 0..10 {
        frame(&mut w, &cmd());
    }
    assert!(!w.ply.wallhit_active);
    assert_eq!(w.tile(15, 12), 0x3C);
    assert_eq!(w.mapspot(15, 12, 1), 0x69, "see-through after the break");
    let s = sounds(&take_events(&mut w));
    assert!(s.is_empty(), "wall-break sounds are positional");
}

/// Data-gated: the transcribed fixture matches `attackinfo` in `COUNT.EXE`.
#[test]
fn attackinfo_fixture_matches_count_exe() {
    let Some(dir) = obc_testdata::data_dir() else {
        eprintln!("SKIPPED: original game data not found (set OBC_DATA_DIR)");
        return;
    };
    let exe = MzExe::parse(DataDir::open(dir).unwrap().read("COUNT.EXE").unwrap()).unwrap();
    let data = GameData::load(&exe).unwrap();
    for (wpn, row) in ATTACKINFO.iter().enumerate() {
        for (k, &(tics, attack, frame)) in row.iter().enumerate() {
            let s = data.attackinfo[wpn][k];
            assert_eq!(
                (s.tics, s.attack, s.frame),
                (tics, attack, frame),
                "weapon {wpn} entry {k}"
            );
        }
    }
    let p = data.state(S_PLAYER).unwrap();
    assert_eq!(p.think, Some(obc_sim::data::Func::TPlayer));
    assert_eq!(data.state(S_ATTACK).unwrap().think, Some(obc_sim::data::Func::TAttack));
}
