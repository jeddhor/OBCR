//! `check_map` (docs/MODS_AND_OBCEDIT.md §3.3): it accepts every original
//! floor (data-gated) and refuses what the engine cannot play.

use obc_formats::DataDir;
use obc_formats::maps::{Compression, Map, MapArchive};
use obc_sim::level::{Severity, check_map};

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
