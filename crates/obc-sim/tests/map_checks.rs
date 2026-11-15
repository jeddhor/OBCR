//! `check_map` (docs/MODS_AND_OBCEDIT.md §3.3): it accepts every original
//! floor (data-gated) and refuses what the engine cannot play.

use obc_formats::DataDir;
use obc_formats::maps::{Compression, Map, MapArchive};
use obc_sim::level::{Severity, check_arena, check_map};

fn blank() -> Map {
    let n = 64 * 64;
    let mut p0 = vec![144u16; n];
    for i in 0..64 {
        for j in [i, 63 * 64 + i, i * 64, i * 64 + 63] {
            p0[j] = 1;
        }
    }
    let mut p1 = vec![18u16; n];
    p1[2 * 64 + 2] = 20;
    Map { name: "T".into(), width: 64, height: 64, planes: [p0, p1, vec![0; n]] }
}

fn errors(m: &Map) -> Vec<String> {
    check_map(m).into_iter().filter(|p| p.severity == Severity::Error).map(|p| p.message).collect()
}

#[test]
fn a_blank_room_is_fine_and_mistakes_are_caught() {
    let m = blank();
    assert!(check_map(&m).is_empty(), "{:?}", check_map(&m));
    let mut two = blank();
    two.planes[1][5 * 64 + 5] = 21;
    assert!(errors(&two)[0].contains("2 player starts"));
    let mut none = blank();
    none.planes[1][2 * 64 + 2] = 18;
    assert!(errors(&none)[0].contains("no player start"));
    let mut bad = blank();
    bad.planes[0][10 * 64 + 10] = 170;
    assert!(errors(&bad)[0].contains("plane-0 code 170"));
    let mut crowd = blank();
    for i in 0..120 {
        crowd.planes[1][(10 + i / 40) * 64 + 10 + i % 40] = 116; // soldiers
    }
    assert!(errors(&crowd).iter().any(|e| e.contains("actors")));
}

#[test]
fn every_original_floor_passes() {
    let Some(dir) = obc_testdata::data_dir() else {
        eprintln!("SKIPPED: original game data not found (set OBC_DATA_DIR)");
        return;
    };
    let d = DataDir::open(dir).unwrap();
    let maps = MapArchive::parse(
        &d.read("MAPHEAD.BC").unwrap(),
        d.read("MAPTEMP.BC").unwrap(),
        Compression::Rlew,
    )
    .unwrap();
    for slot in 0..40 {
        let map = maps.map(slot).unwrap().unwrap();
        let e = errors(&map);
        assert!(e.is_empty(), "floor {}: {e:?}", slot + 1);
    }
}

#[test]
fn arenas_refuse_enemies_and_warn_of_cut_off_floor() {
    let m = blank();
    assert!(check_arena(&m).is_empty(), "{:?}", check_arena(&m));
    let mut enemy = blank();
    enemy.planes[1][10 * 64 + 10] = 116;
    let p = check_arena(&enemy);
    assert!(
        p.iter().any(|p| p.severity == Severity::Error && p.message.contains("1 enemies")),
        "{p:?}"
    );
    // A wall across the room cuts the east half off from the start.
    let mut split = blank();
    for y in 0..64 {
        split.planes[0][y * 64 + 32] = 1;
    }
    let p = check_arena(&split);
    let w: Vec<_> = p.iter().filter(|p| p.severity == Severity::Warning).collect();
    assert_eq!(w.len(), 1, "{p:?}");
    assert!(w[0].message.contains("31 floor cells") || w[0].message.contains("cannot be reached"));
    assert_eq!(w[0].at, Some((33, 1)));
    // A door in the wall joins them again.
    split.planes[0][10 * 64 + 32] = 130;
    split.planes[0][9 * 64 + 32] = 1;
    split.planes[0][11 * 64 + 32] = 1;
    assert!(
        check_arena(&split).iter().all(|p| !p.message.contains("reached")),
        "{:?}",
        check_arena(&split)
    );
}

#[test]
fn every_original_arena_passes_the_arena_checks() {
    let Some(dir) = obc_testdata::data_dir() else {
        eprintln!("SKIPPED: original game data not found (set OBC_DATA_DIR)");
        return;
    };
    let d = DataDir::open(dir).unwrap();
    let maps = MapArchive::parse(
        &d.read("MAPHEAD.BC").unwrap(),
        d.read("MAPTEMP.BC").unwrap(),
        Compression::Rlew,
    )
    .unwrap();
    for slot in 50..60 {
        let map = maps.map(slot).unwrap().unwrap();
        let e: Vec<_> =
            check_arena(&map).into_iter().filter(|p| p.severity == Severity::Error).collect();
        assert!(e.is_empty(), "arena slot {slot}: {e:?}");
    }
}
