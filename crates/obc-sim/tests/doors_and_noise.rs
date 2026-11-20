//! Data-gated: GitHub issues #10 and #12.
//!
//! - #10: floor 4's sewer fat man stands on an ambush tile (58,10), so his
//!   area is 254; the original reads `areabyplayer[254]` out of bounds, from
//!   `actorat`, and he hears every shot (quirk K-005, reproduced).
//! - #12: an opened door-look wall blocks shots and sight in the original
//!   (quirk K-004); its opt-in fix lets them through.
//!
//! Each is shown on the original map, and again on the reporter's own beta-4
//! save when it is present (`.local/issues`, never committed).

use std::path::PathBuf;
use std::sync::Arc;

use obc_core::trig::Tables;
use obc_formats::DataDir;
use obc_formats::exe::MzExe;
use obc_formats::maps::{Compression, MapArchive};
use obc_sim::actors::check_line;
use obc_sim::data::GameData;
use obc_sim::fixes::K_004;
use obc_sim::world::{NUMBUTTONS, ObjId, TicCmd, World, bt};

fn data() -> Option<(Arc<GameData>, MapArchive)> {
    let dir = DataDir::open(obc_testdata::data_dir()?).ok()?;
    let exe = MzExe::parse(dir.read("COUNT.EXE").ok()?).ok()?;
    let maps = MapArchive::parse(
        &dir.read("MAPHEAD.BC").ok()?,
        dir.read("MAPTEMP.BC").ok()?,
        Compression::Rlew,
    )
    .ok()?;
    Some((Arc::new(GameData::load(&exe).ok()?), maps))
}

/// A new game on map `slot`.
fn level(slot: usize) -> Option<World> {
    let (data, maps) = data()?;
    let mut w = World::new(data, Arc::new(Tables::build()));
    obc_sim::player::new_game(&mut w, 2);
    w.setup_game_level(&maps.map(slot).ok()??, slot as i16);
    Some(w)
}

/// The reporter's save `name`, if `.local/issues` holds it.
fn reporter_save(name: &str) -> Option<World> {
    let (data, _) = data()?;
    let here = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let path =
        here.ancestors().map(|a| a.join(".local/issues").join(name)).find(|p| p.is_file())?;
    let (_, mut w) =
        obc_sim::save::read(&std::fs::read(path).ok()?, data, Arc::new(Tables::build())).ok()?;
    w.loadedgame = false;
    Some(w)
}

fn find(w: &World, class: u16, x: u16, y: u16) -> ObjId {
    let mut cur = w.plr().next;
    while let Some(id) = cur {
        let o = w.obj(id);
        if o.obclass == class && (o.tilex, o.tiley) == (x, y) {
            return id;
        }
        cur = o.next;
    }
    panic!("no class {class} at ({x},{y})");
}

fn fire() -> TicCmd {
    let mut buttons = [false; NUMBUTTONS];
    buttons[bt::ATTACK] = true;
    TicCmd { buttons, ..TicCmd::default() }
}

/// Shoots a few times, then waits; whether the fat man at (58,10) woke.
fn fat_man_wakes(w: &mut World) -> bool {
    let fat = find(w, 17, 58, 10);
    let stand = w.obj(fat).state;
    assert_eq!(w.obj(fat).areanumber, 254, "spawned on an ambush tile");
    for _ in 0..30 {
        w.frame(&fire(), 2);
    }
    for _ in 0..300 {
        w.frame(&TicCmd::default(), 2);
        if w.obj(fat).state != stand {
            return true;
        }
    }
    false
}

#[test]
fn the_floor_4_sewer_man_hears_shots_as_in_dos() {
    let Some(mut w) = level(3) else {
        eprintln!("SKIPPED: original game data not found (set OBC_DATA_DIR)");
        return;
    };
    assert!(!w.actorat(0, 30).is_empty(), "the west-edge wall the original reads");
    assert!(w.area_by_player(254));
    // From the start of floor 4, far from his room: a shot anywhere wakes him.
    assert!(fat_man_wakes(&mut w), "he should wake after the shots");
    if let Some(mut w) = reporter_save("i10-port/svgm4.obcr") {
        assert!(fat_man_wakes(&mut w), "the reporter's save");
    }
}

/// On floor 6 (slot 5), the door-look wall at (10,48) and the soldier
/// behind it at (10,50): opened, with the player at (10,46).
fn opened_door_scene() -> Option<(World, ObjId)> {
    let mut w = level(5)?;
    let enemy = find(&w, 4, 10, 50);
    w.operate_anim_wall(10, 48);
    for _ in 0..200 {
        w.move_anim_walls();
    }
    let i = w.anim_wall_at(10, 48).expect("the door-look wall");
    assert!(matches!(w.animwalls[i].state, 0 | 4), "opened");
    let p = w.player;
    let o = w.obj_mut(p);
    o.x = (10 << 16) | 0x8000;
    o.y = (46 << 16) | 0x8000;
    o.tilex = 10;
    o.tiley = 46;
    o.angle = 270;
    let (x, y) = (o.x, o.y);
    w.plux = (x >> 8) as i16;
    w.pluy = (y >> 8) as i16;
    Some((w, enemy))
}

#[test]
fn an_open_door_wall_blocks_sight_unless_fixed() {
    let Some((mut w, enemy)) = opened_door_scene() else {
        eprintln!("SKIPPED: original game data not found (set OBC_DATA_DIR)");
        return;
    };
    assert!(!check_line(&w, enemy), "the original: no line through the open wall");
    w.fixes.set(&K_004, true);
    assert!(check_line(&w, enemy), "K-004 fixed: the line passes");
}

#[test]
fn shots_reach_the_enemy_behind_an_open_door_wall_only_when_fixed() {
    if data().is_none() {
        eprintln!("SKIPPED: original game data not found (set OBC_DATA_DIR)");
        return;
    }
    // On the reporter's save when present, else the rebuilt scene.
    let damage = |fixed: bool| {
        let (mut w, enemy) = match reporter_save("i12/svgm8.obcr") {
            Some(w) => {
                let e = find(&w, 4, 10, 50);
                (w, e)
            }
            None => opened_door_scene().unwrap(),
        };
        w.fixes.set(&K_004, fixed);
        let before = w.obj(enemy).hitpoints;
        for _ in 0..20 {
            w.frame(&fire(), 2);
        }
        before - w.obj(enemy).hitpoints
    };
    assert_eq!(damage(false), 0, "unfixed: shots stop at the wall");
    assert!(damage(true) > 0, "fixed: shots reach the enemy");
}
