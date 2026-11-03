//! Pickups, the supply crate, damage, healing and score (docs/re/PLAYER.md §4).

mod player_support;

use obc_core::rng::RNDTABLE;
use obc_sim::events::SimEvent;
use obc_sim::player;
use obc_sim::world::{Static, World, ex, fl};
use player_support::*;

fn world() -> World {
    let mut w = room_world();
    start(&mut w, 1, 12, 12, 1);
    take_events(&mut w);
    w
}

/// Adds a bonus static with `item` and returns its index.
fn item(w: &mut World, item: i16) -> usize {
    w.statics.push(Static { tilex: 12, tiley: 12, shape: 30, flags: fl::BONUS, item });
    w.statics.len() - 1
}

/// Sets the RNG so that the next `US_RndT` returns `value`.
fn next_rnd(w: &mut World, value: u8) {
    let i = RNDTABLE.iter().skip(1).position(|&v| v == value).expect("value in table") + 1;
    w.rng.index = (i - 1) as u8;
}

#[test]
fn flak_jacket_and_increased_health() {
    let mut w = world();
    let i = item(&mut w, 8);
    player::get_bonus(&mut w, i);
    assert_eq!(w.gamestate.armor, 100);
    assert_eq!(w.statics[i].shape, -1);
    let ev = take_events(&mut w);
    assert_eq!(messages(&ev), ["Flak Jacket"]);
    assert!(ev.contains(&SimEvent::ItemIcon(8)));
    // At 400 or more the jacket stays on the floor.
    w.gamestate.armor = 400;
    let i = item(&mut w, 8);
    player::get_bonus(&mut w, i);
    assert_eq!((w.gamestate.armor, w.statics[i].shape), (400, 30));

    w.gamestate.health = 350;
    let i = item(&mut w, 9);
    player::get_bonus(&mut w, i);
    assert_eq!((w.gamestate.health, w.gamestate.maxhealth), (450, 450));
    let i = item(&mut w, 9);
    player::get_bonus(&mut w, i);
    assert_eq!(w.gamestate.health, 450, "not taken at 400 or more");
    assert_eq!(w.statics[i].shape, 30);
}

#[test]
fn weapons_and_clips() {
    let mut w = world();
    let i = item(&mut w, 5);
    player::get_bonus(&mut w, i);
    assert_eq!(w.gamestate.weapon, 2);
    assert_eq!(w.gamestate.clips[1], 2, "GiveWeapon's clip plus GiveClip");
    let ev = take_events(&mut w);
    assert_eq!(messages(&ev), ["Galil"]);
    assert_eq!(sounds(&ev), [85]);

    let i = item(&mut w, 11);
    player::get_bonus(&mut w, i);
    assert_eq!(w.gamestate.clips[0], 3);
    w.gamestate.clips[0] = 19;
    let i = item(&mut w, 11);
    player::get_bonus(&mut w, i);
    assert_eq!(w.statics[i].shape, 30, "Uzi clip refused at 19");

    let i = item(&mut w, 13);
    player::get_bonus(&mut w, i);
    assert_eq!((w.gamestate.weapon, w.gamestate.clips[3]), (4, 2));
    let i = item(&mut w, 4);
    player::get_bonus(&mut w, i);
    assert_eq!(w.gamestate.clips[3], 3);

    w.gamestate.ammo = 4;
    let i = item(&mut w, 14);
    player::get_bonus(&mut w, i);
    assert_eq!((w.gamestate.clips[2], w.gamestate.fuel_cap, w.gamestate.ammo), (1, 200, 30));
}

#[test]
fn thermosensor_and_unknown_items() {
    let mut w = world();
    let i = item(&mut w, 10);
    player::get_bonus(&mut w, i);
    assert!(w.thermosensor);
    for n in [0, 1, 2, 6, 7, 15, 19, 21] {
        let i = item(&mut w, n);
        player::get_bonus(&mut w, i);
        assert_eq!(w.statics[i].shape, 30, "item {n} is not a pickup");
    }
}

#[test]
fn supply_crate_outcomes() {
    // r = US_RndT() + 1 = 9 → grenades; n = 109 / 64 + 1 = 2 (RNDTABLE[1..]).
    let mut w = world();
    let i = item(&mut w, 3);
    w.rng.index = 0;
    player::get_bonus(&mut w, i);
    assert_eq!(w.gamestate.clips[3], 2);
    assert_eq!(w.gamestate.maxammo, 230);
    let ev = take_events(&mut w);
    assert_eq!(messages(&ev), [" 2 Grenades"]);
    assert!(ev.contains(&SimEvent::ItemIcon(3)));

    // r = 221 → 9 mm clips; n = 222 / 37 + 1 = 7.
    let mut w = world();
    let i = item(&mut w, 3);
    w.rng.index = 2;
    player::get_bonus(&mut w, i);
    assert_eq!(w.gamestate.clips[0], 2 + 7);
    let ev = take_events(&mut w);
    assert_eq!(messages(&ev), [" 7 9mm Clips"]);
    assert_eq!(sounds(&ev), [85]);

    // r = 1 → a fuel clip, silently, below the cap.
    let mut w = world();
    let i = item(&mut w, 3);
    next_rnd(&mut w, 0);
    player::get_bonus(&mut w, i);
    assert_eq!(w.gamestate.clips[2], 1);
    assert!(messages(&take_events(&mut w)).is_empty());
    // … and with the message and a refill at the cap.
    w.gamestate.clips[2] = 18;
    w.gamestate.ammo = 0;
    let i = item(&mut w, 3);
    next_rnd(&mut w, 0);
    player::get_bonus(&mut w, i);
    assert_eq!((w.gamestate.clips[2], w.gamestate.ammo), (19, 30));
    assert_eq!(messages(&take_events(&mut w)), ["Fuel Bottle"]);

    // r in 21..=64 → 7 mm clips, capped at 19.
    let mut w = world();
    w.gamestate.clips[1] = 18;
    let i = item(&mut w, 3);
    next_rnd(&mut w, 47);
    player::get_bonus(&mut w, i);
    assert_eq!(w.gamestate.clips[1], 19);
}

#[test]
fn booby_trap() {
    let mut w = world();
    w.gamestate.armor = 0;
    let i = item(&mut w, 20);
    next_rnd(&mut w, 50);
    player::get_bonus(&mut w, i);
    // TakeDamage(50) at difficulty 1: ×1, then ×2.
    assert_eq!(w.gamestate.health, 300);
    assert_eq!(w.ply.knockback_trigger, 10);
    assert_eq!(w.statics[i].shape, -1);
    let ev = take_events(&mut w);
    assert!(ev.contains(&SimEvent::Shake(4)));
    assert_eq!(messages(&ev), ["BOOBY TRAP"]);
    assert_eq!(sounds(&ev), [75]);
}

#[test]
fn take_damage_armor_and_difficulty() {
    // Difficulty 1: ×1 (health ≥ 30), armor takes half, then ×2.
    let mut w = world();
    w.gamestate.armor = 100;
    player::take_damage(&mut w, 20, None);
    assert_eq!((w.gamestate.health, w.gamestate.armor), (380, 90));

    // Difficulty 0: start armor 200; ×1, half to armor, ×2.
    let mut w = room_world();
    start(&mut w, 0, 12, 12, 1);
    assert_eq!(w.gamestate.armor, 200);
    player::take_damage(&mut w, 21, None);
    assert_eq!((w.gamestate.health, w.gamestate.armor), (400 - 20, 190));

    // Difficulty 2: ×2 then ×4 = ×8 while health ≥ 30, ×4 below.
    let mut w = room_world();
    start(&mut w, 2, 12, 12, 1);
    player::take_damage(&mut w, 10, None);
    assert_eq!(w.gamestate.health, 320);
    w.gamestate.health = 29;
    w.rng.index = 1; // the blackout roll draws 109: no fade
    player::take_damage(&mut w, 5, None);
    assert_eq!(w.gamestate.health, 9);

    // Difficulty 3: ×2 then ×3.
    let mut w = room_world();
    start(&mut w, 3, 12, 12, 1);
    player::take_damage(&mut w, 10, None);
    assert_eq!(w.gamestate.health, 340);

    // Armor never caps the absorbed half.
    let mut w = world();
    w.gamestate.armor = 3;
    player::take_damage(&mut w, 40, None);
    assert_eq!((w.gamestate.health, w.gamestate.armor), (360, 0));
}

#[test]
fn take_damage_god_mode_immunity_and_death() {
    let mut w = world();
    w.godmode = true;
    player::take_damage(&mut w, 100, None);
    assert_eq!(w.gamestate.health, 400);
    assert!(take_events(&mut w).contains(&SimEvent::DamageFlash(6)), "the flash still shows");

    let mut w = world();
    w.gamestate.immune = 1;
    player::take_damage(&mut w, 100, None);
    assert_eq!(w.gamestate.health, 400);

    let mut w = world();
    let enemy = w.get_new_actor().unwrap();
    w.gamestate.health = 40;
    w.rng.index = 1;
    player::take_damage(&mut w, 100, Some(enemy));
    assert_eq!(w.gamestate.health, 0);
    assert_eq!(w.playstate, ex::DIED);
    assert_eq!(w.killerobj, Some(enemy));
    assert_eq!(w.lastattacker, Some(enemy));
}

#[test]
fn low_health_blackout_roll() {
    // Health ≤ 35 after the hit: one US_RndT; 1 in 15 fades to black.
    let mut w = world();
    w.gamestate.health = 40;
    next_rnd(&mut w, 0);
    player::take_damage(&mut w, 1, None);
    assert_eq!(w.gamestate.health, 38);
    assert_eq!(w.rng.index, 82, "no roll above 35");
    w.gamestate.health = 30;
    next_rnd(&mut w, 0);
    player::take_damage(&mut w, 1, None);
    assert!(take_events(&mut w).contains(&SimEvent::Blackout));
}

#[test]
fn heal_self_clamps_down_to_maxhealth() {
    let mut w = world();
    assert_eq!(w.gamestate.maxhealth, 100);
    player::heal_self(&mut w, 10);
    assert_eq!(w.gamestate.health, 100, "an unguarded heal lowers health 400 → 100");
    w.gamestate.health = 50;
    player::heal_self(&mut w, 25);
    assert_eq!(w.gamestate.health, 75);
}

#[test]
fn give_points_caps_the_score() {
    let mut w = world();
    player::give_points(&mut w, 500);
    assert_eq!(w.gamestate.score, 500);
    player::give_points(&mut w, 2_000_000);
    assert_eq!(w.gamestate.score, 1_000_000);
    // Network games score elsewhere.
    let mut w = room_world();
    start(&mut w, 4, 12, 12, 1);
    player::give_points(&mut w, 500);
    assert_eq!(w.gamestate.score, 0);
}

#[test]
fn health_cap_and_level_setup() {
    let mut w = world();
    w.gamestate.health = 1200;
    player::post_actors(&mut w);
    assert_eq!(w.gamestate.health, 999);
    w.gamestate.maxhealth = 500;
    w.gamestate.shots_level = 7;
    player::setup_level_player_state(&mut w);
    assert_eq!((w.gamestate.health, w.gamestate.maxhealth), (400, 100));
    assert_eq!((w.gamestate.shots_level, w.gamestate.shots_total), (0, 7));
}

#[test]
fn screen_splats_consume_random_numbers() {
    let mut w = world();
    w.rng.index = 0;
    player::add_screen_splat(&mut w, 2);
    assert_eq!(w.rng.index, 3, "shape, x, y");
    let s = w.ply.splats[0];
    assert_eq!(s.shape, 8 % 8);
    assert_eq!(s.x, 109, "within 64..viewwidth-64");
    assert_eq!(s.y, 128, "RNDTABLE[3] = 220 clamped to viewheight");
    assert_eq!(s.kind, 2);
    // Type 1 adds a splat sound (one more draw).
    player::add_screen_splat(&mut w, 1);
    assert_eq!(w.rng.index, 7);
    assert_eq!(w.ply.splatcount, 2);
    for _ in 0..30 {
        player::add_screen_splat(&mut w, 0);
    }
    assert_eq!(w.ply.splatcount, 19);
    let idx = w.rng.index;
    player::add_screen_splat(&mut w, 0);
    assert_eq!(w.rng.index, idx, "a full visor draws nothing");
    player::clear_screen_splats(&mut w);
    assert_eq!(w.ply.splatcount, 0);
}
