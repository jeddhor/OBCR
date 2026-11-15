//! `obc-tool catalog`: the OBCEdit catalog (docs/MODS_AND_OBCEDIT.md §5.4).
//!
//! Every map value OBCEdit can paint, with its meaning, generated from the
//! code that gives it that meaning: plane-1 codes go through
//! [`obc_sim::level::classify_info`] and are spawned into a scratch world by
//! the real `SetupGameLevel`, so the class, the sprite and the difficulty tier
//! are whatever the engine does with them. Plane-0 meanings follow
//! docs/re/LEVEL_CODES.md §3 and the engine's own constants.
//!
//! The output holds names, kinds, small integers (wall pages, sprite numbers,
//! statinfo kinds) and limits: no pixels, palettes or map data. Sprite
//! numbers index the sprite list as OBCR's renderer does (`sprites[shape]`).

use std::sync::Arc;

use anyhow::{Context, Result};
use obc_core::trig::Tables;
use obc_formats::DataDir;
use obc_formats::exe::MzExe;
use obc_formats::maps::{Map, NUM_PLANES};
use obc_formats::release;
use obc_formats::vswap::PageFile;
use obc_sim::data::{ExeLayout, GameData};
use obc_sim::level::{InfoCode, LevelProfile, classify_info};
use obc_sim::world::{AREATILE, MAPSIZE, MAXACTORS, NUMAREAS, World, class};
use serde_json::{Value, json};

/// Schema version of the catalog file; OBCEdit refuses a newer one.
pub const SCHEMA_VERSION: u32 = 1;

const COMPASS: [&str; 4] = ["north", "east", "south", "west"];
/// Actor `dirtype / 2`: 0 E, 1 N, 2 W, 3 S.
const ACTOR_DIRS: [&str; 4] = ["east", "north", "west", "south"];
/// Patrol arrows 90-97: `dirtype` 0 E .. 7 SE.
const ARROWS: [&str; 8] =
    ["east", "northeast", "north", "northwest", "west", "southwest", "south", "southeast"];

fn class_name(c: u16) -> &'static str {
    match c {
        class::RAT => "Rat",
        class::SOLDIER => "Soldier",
        class::ALLY => "Ally",
        class::SAUCER => "Saucer drone",
        class::GIANT_RAT => "Giant rat",
        class::STOOL => "Soldier on a stool",
        class::CROUCH => "Crouching soldier",
        class::TOILET => "Soldier on a toilet",
        class::PRONE => "Prone soldier",
        class::BLACK_SUIT => "Man in a black suit",
        class::BOSS => "Victor (final boss)",
        class::GUNNER => "Gunner",
        class::RED_VEST => "Red-vest brute",
        class::RED_GIANT => "Red giant",
        class::FAT_MAN => "Fat man",
        class::FIRE => "Fire",
        class::MANHOLE => "Manhole fat man",
        class::SWAMP => "Swamp creature",
        _ => "Actor",
    }
}

/// `statinfo` kinds that are pickups (`GetBonus`), by name (LEVEL_CODES §4.3).
fn bonus_name(kind: i16) -> Option<&'static str> {
    Some(match kind {
        3 => "Ammo",
        4 => "Grenades",
        5 => "Galil",
        8 => "Flak jacket",
        9 => "Increased health",
        10 => "Map ThermoSensor",
        11 => "Uzi",
        12 => "Flame thrower",
        13 => "Grenade launcher",
        14 => "Fuel pack",
        20 => "Booby trap",
        _ => return None,
    })
}

/// A 64×64 map: walls round the edge, floor 144 inside, the player start at
/// (1, 1) facing east, and `code` on plane 1 at (32, 32).
fn probe_map(code: u16) -> Map {
    let n = MAPSIZE * MAPSIZE;
    let mut p0 = vec![144u16; n];
    for i in 0..MAPSIZE {
        p0[i] = 1;
        p0[(MAPSIZE - 1) * MAPSIZE + i] = 1;
        p0[i * MAPSIZE] = 1;
        p0[i * MAPSIZE + MAPSIZE - 1] = 1;
    }
    let mut p1 = vec![18u16; n];
    if !(19..=22).contains(&code) {
        p1[MAPSIZE + 1] = 20;
    }
    p1[32 * MAPSIZE + 32] = code;
    let mut planes: [Vec<u16>; NUM_PLANES] = Default::default();
    planes[0] = p0;
    planes[1] = p1;
    planes[2] = vec![0; n];
    Map { name: "PROBE".into(), width: MAPSIZE as u16, height: MAPSIZE as u16, planes }
}

/// What the engine spawns for a plane-1 code: (class, sprite, rotates).
fn spawned_actor(data: &Arc<GameData>, trig: &Arc<Tables>, code: u16) -> Option<(u16, i16, bool)> {
    let mut w = World::new(data.clone(), trig.clone());
    obc_sim::player::new_game(&mut w, 2);
    w.setup_game_level_with(&probe_map(code), 6, LevelProfile::stock(6, data.cd));
    let player = w.player;
    w.objs.iter().enumerate().find_map(|(i, o)| {
        if o.state == 0 || i == usize::from(player) {
            return None;
        }
        let st = data.state(o.state)?;
        Some((o.obclass, st.shape, st.rotate != 0))
    })
}

fn plane0_entry(v: u16) -> Value {
    let wall = |name: String, kind: &str, extra: Value| {
        let mut e = json!({
            "value": v, "kind": kind, "name": name, "page": v - 1, "solid": true,
        });
        if let (Value::Object(m), Value::Object(x)) = (&mut e, extra) {
            m.extend(x);
        }
        e
    };
    match v {
        0 => json!({"value": 0, "kind": "invalid", "name": "Nothing (invalid)", "solid": false}),
        13 | 29 | 49 | 65 | 69 => wall(
            format!("Door-look wall {v}"),
            "door_look",
            json!({"placeable": true, "frames": [v, v + 1, v + 2, v + 3]}),
        ),
        14..=16 | 30..=32 | 50..=52 | 66..=68 | 70..=72 => wall(
            format!("Door-look wall {v} (animation frame)"),
            "door_look_frame",
            json!({"placeable": false}),
        ),
        57 => wall("Glass pane".into(), "glass", json!({"broken": 60})),
        58 | 59 => {
            wall(format!("Glass pane (cracking, {v})"), "glass_frame", json!({"placeable": false}))
        }
        60 => wall("Glass pane (broken)".into(), "glass_broken", json!({})),
        117 => wall("Screen pane".into(), "glass", json!({"broken": 120})),
        118 | 119 => {
            wall(format!("Screen pane (cracking, {v})"), "glass_frame", json!({"placeable": false}))
        }
        120 => wall("Screen pane (broken)".into(), "glass_broken", json!({})),
        73 => wall("Porthole fan".into(), "fan", json!({"frames": [73, 74, 75, 76]})),
        74..=76 => wall(
            format!("Porthole fan (frame {})", v - 73),
            "fan_frame",
            json!({"placeable": false}),
        ),
        77 => wall("Damaged wall".into(), "damaged", json!({})),
        85 => wall("Special panel".into(), "panel", json!({})),
        88 => wall("Rock opening (sewer)".into(), "rock_opening", json!({"needs_plane1": 107})),
        95 => wall("Generator switch (lights on)".into(), "generator", json!({"on": true})),
        96 => wall(
            "Generator switch (lights off: the floor starts dark)".into(),
            "generator",
            json!({"on": false}),
        ),
        113 => wall("Exit: ladder".into(), "exit", json!({"exit": "ladder", "exitkind": 0})),
        114 => wall("Exit: stairs".into(), "exit", json!({"exit": "stairs", "exitkind": 0})),
        115 => wall("Exit: elevator".into(), "exit", json!({"exit": "elevator", "exitkind": 1})),
        1..=129 => wall(format!("Wall {v}"), "wall", json!({})),
        130..=137 => {
            let vertical = v.is_multiple_of(2);
            let lock = (v - 130) / 2;
            // HitHorizDoor/HitVertDoor tables (LEVEL_CODES §3.4): DOORWALL 128 + {0, 4, 5, 4}.
            let page = 128 + [0u16, 4, 5, 4][usize::from(lock)];
            json!({
                "value": v, "kind": "door", "solid": true,
                "name": format!("Door, look {} ({})", lock + 1, if vertical { "vertical" } else { "horizontal" }),
                "lock": lock, "vertical": vertical, "page": page,
            })
        }
        138 => json!({"value": 138, "kind": "ambush", "name": "Ambush marker", "solid": false,
            "note": "Under a standing enemy: cleared to the neighbouring floor, the enemy waits in ambush"}),
        139 => json!({"value": 139, "kind": "unused", "name": "Unused solid code", "solid": true}),
        _ => {
            let area = v - AREATILE;
            let set = v.checked_sub(144).filter(|s| *s < 10);
            json!({
                "value": v, "kind": "floor", "solid": false,
                "name": match set {
                    Some(s) => format!("Floor, area {area}, texture set {s}"),
                    None => format!("Floor, area {area}"),
                },
                "area": area, "floor_set": set,
                "automap_flag": v == 143,
            })
        }
    }
}

fn plane1_entries(data: &Arc<GameData>, trig: &Arc<Tables>, num_sprites: usize) -> Vec<Value> {
    let mut out = vec![json!({"value": 18, "kind": "empty", "name": "Empty (background)"})];
    for code in 19..=317u16 {
        let (what, tier) = classify_info(code);
        let difficulty = json!({"min_game_type": tier, "tier": tier});
        let e = match what {
            InfoCode::Player { dir } => json!({
                "kind": "player", "name": format!("Player start, facing {}", COMPASS[usize::from(dir)]),
                "facing": COMPASS[usize::from(dir)],
            }),
            InfoCode::Static { index } => {
                let Some(&info) = data.statinfo.get(usize::from(index)) else { continue };
                // The one difference between the releases (CD_RELEASE.md §2):
                // entry 48 is passable on the floppy and blocks on the CD. The
                // catalog records both, so it is the same from either release.
                let info =
                    if code == 71 { obc_sim::data::StatInfo { kind: 0, ..info } } else { info };
                let blocking = matches!(info.kind, 1 | 17);
                let name = bonus_name(info.kind).map_or_else(
                    || format!("{} {}", if blocking { "Obstacle" } else { "Decoration" }, code),
                    str::to_owned,
                );
                let mut e = json!({
                    "kind": if bonus_name(info.kind).is_some() { "pickup" } else { "static" },
                    "name": name, "sprite": info.shape, "statinfo_kind": info.kind,
                    "blocking": blocking,
                });
                if code == 71 {
                    e["blocking_cd"] = json!(true);
                    e["name"] = json!("Decoration 71 (blocks on the CD)");
                }
                e
            }
            InfoCode::AnimWall { kind } => {
                let (name, on) = match code {
                    86 => ("Fan speed: slow".to_owned(), "fan"),
                    87 => ("Fan speed: medium".to_owned(), "fan"),
                    88 => ("Fan speed: fast".to_owned(), "fan"),
                    103 | 124 => (format!("Sliding panel ({code})"), "door_look"),
                    106 => ("Sliding see-through panel".to_owned(), "door_look"),
                    _ => ("Thin open passage".to_owned(), "opening"),
                };
                json!({"kind": "anim_wall", "name": name, "anim_type": kind, "placed_on": on,
                    "see_through": matches!(code, 106 | 107), "unused_in_maps": code == 124})
            }
            InfoCode::PushWall => {
                json!({"kind": "push_wall", "name": "Push wall (secret)", "travel_cells": 2})
            }
            InfoCode::Nothing => match code {
                89 => {
                    json!({"kind": "marker", "name": "Explosion-proof wall", "marker": "indestructible"})
                }
                90..=97 => json!({
                    "kind": "patrol_arrow", "name": format!("Patrol turn: {}", ARROWS[usize::from(code - 90)]),
                    "direction": ARROWS[usize::from(code - 90)],
                }),
                104 => {
                    json!({"kind": "marker", "name": "Window (see-through, blocks enemy sight)", "marker": "window"})
                }
                105 => {
                    json!({"kind": "marker", "name": "Grille (see-through for all)", "marker": "grille"})
                }
                125 => {
                    json!({"kind": "marker", "name": "Door-frame wall (damage only)", "marker": "door_frame"})
                }
                _ => continue,
            },
            InfoCode::Fire => json!({"kind": "fire", "name": "Fire", "class": class::FIRE}),
            InfoCode::Stand { dir, .. } | InfoCode::Patrol { dir, .. } | InfoCode::Ally { dir } => {
                let Some((cls, shape, rotates)) = spawned_actor(data, trig, code) else { continue };
                let patrol = matches!(what, InfoCode::Patrol { .. });
                let mut name = format!(
                    "{}, {} {}",
                    class_name(cls),
                    if patrol { "patrolling" } else { "standing" },
                    ACTOR_DIRS[usize::from(dir)]
                );
                if let InfoCode::Patrol { which: 3, hover, .. } = what {
                    name = format!(
                        "{}, {} {}",
                        class_name(cls),
                        if hover { "hovering" } else { "moving" },
                        ACTOR_DIRS[usize::from(dir)]
                    );
                }
                json!({
                    "kind": if cls == class::ALLY { "ally" } else { "enemy" },
                    "name": name, "class": cls, "class_name": class_name(cls),
                    "patrol": patrol, "facing": ACTOR_DIRS[usize::from(dir)],
                    "sprite": shape, "rotates": rotates,
                })
            }
            InfoCode::Special { .. } => {
                let Some((cls, shape, rotates)) = spawned_actor(data, trig, code) else { continue };
                json!({
                    "kind": "enemy", "name": class_name(cls), "class": cls, "class_name": class_name(cls),
                    "patrol": false, "sprite": shape, "rotates": rotates,
                    "ends_game": cls == class::BOSS,
                })
            }
        };
        let mut e = e;
        if let Value::Object(m) = &mut e {
            m.insert("value".into(), json!(code));
            if let Some(s) = m.get("sprite").and_then(Value::as_i64)
                && s >= num_sprites as i64
            {
                m.insert("sprite".into(), Value::Null);
            }
            if matches!(
                what,
                InfoCode::Stand { .. } | InfoCode::Patrol { .. } | InfoCode::Ally { .. }
            ) {
                m.insert("difficulty".into(), difficulty);
            }
        }
        out.push(e);
    }
    out
}

/// Builds the catalog from the game data in `data`.
pub fn build(data: &DataDir) -> Result<Value> {
    let id = release::identify(data);
    let rel = id.release;
    let exe = MzExe::parse(data.read(rel.exe)?)?;
    let game = Arc::new(GameData::load_with(&exe, ExeLayout::for_release(rel.id))?);
    let trig = Arc::new(Tables::build());
    let pf = PageFile::parse(data.read("GFXTILES.BC")?).context("GFXTILES.BC")?;
    let floor_sets: Vec<Value> = game
        .levels
        .floor_sets
        .iter()
        .enumerate()
        .map(|(i, &(f, c))| {
            let sewer = obc_sim::level::SEWER_FLOOR_SETS.get(i);
            json!({
                "set": i, "plane0": 144 + i,
                "floor_page": u16::from(f).saturating_sub(1), "ceiling_page": u16::from(c).saturating_sub(1),
                "sewer_floor_page": sewer.map(|s| u16::from(s.0).saturating_sub(1)),
                "sewer_ceiling_page": sewer.map(|s| u16::from(s.1).saturating_sub(1)),
            })
        })
        .collect();
    Ok(json!({
        "schema": "obcedit-catalog",
        "version": SCHEMA_VERSION,
        "generator": format!("obc-tool catalog {}", env!("CARGO_PKG_VERSION")),
        "note": "Generated from OBCR's code (cargo run -p obc-tool -- catalog). Do not edit by hand.",
        "limits": {
            "map_size": MAPSIZE, "planes": NUM_PLANES, "max_doors": obc_sim::doors::MAXDOORS,
            "max_actors": MAXACTORS, "max_statics": 224,
            "max_anim_walls": obc_sim::animwalls::MAXANIMWALLS, "num_areas": NUMAREAS,
            "areatile": AREATILE, "ambushtile": obc_sim::world::AMBUSHTILE,
            "wall_pages": pf.num_walls(), "max_floor_code": AREATILE as usize + NUMAREAS - 1,
            "map_slots": obc_formats::maps::NUM_MAP_SLOTS,
        },
        "plane0": (0..=(AREATILE + NUMAREAS as u16 - 1)).map(plane0_entry).collect::<Vec<_>>(),
        "plane1": plane1_entries(&game, &trig, pf.num_sprites()),
        "floor_sets": floor_sets,
        "release_differences": [
            {"plane": 1, "value": 71, "note": "Blocks movement in the CD release only (statinfo entry 48)"}
        ],
    }))
}
