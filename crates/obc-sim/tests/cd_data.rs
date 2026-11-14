//! Data-gated (both releases): BCCD.EXE's tables, loaded through the CD
//! layout, line up with COUNT.EXE's (docs/re/CD_RELEASE.md §2).

use obc_formats::DataDir;
use obc_formats::exe::MzExe;
use obc_sim::data::{Func, GameData, OBC_CD};

fn load(dir: std::path::PathBuf, exe: &str, cd: bool) -> GameData {
    let exe = MzExe::parse(DataDir::open(dir).unwrap().read(exe).unwrap()).unwrap();
    if cd { GameData::load_with(&exe, &OBC_CD).unwrap() } else { GameData::load(&exe).unwrap() }
}

#[test]
fn cd_tables_line_up_with_the_floppy_ones() {
    let (Some(fl), Some(cd)) = (obc_testdata::data_dir(), obc_testdata::cd_data_dir()) else {
        eprintln!("SKIPPED: needs both releases (OBC_DATA_DIR and OBC_CD_DATA_DIR)");
        return;
    };
    let a = load(fl, "COUNT.EXE", false);
    let b = load(cd, "BCCD.EXE", true);
    assert!(b.cd && !a.cd);
    // Every CD state resolves, links to a CD state and has a floppy twin.
    for (id, s) in &b.states {
        for f in [s.think, s.action].into_iter().flatten() {
            assert!(!matches!(f, Func::Unknown(_)), "state {id:04x}: {f:?}");
        }
        assert!(s.next == 0 || b.states.contains_key(&s.next), "state {id:04x} -> {:04x}", s.next);
        assert!(a.states.contains_key(id), "state {id:04x} only on the CD");
    }
    // Only the face apparition (0x28BE-0x292E) and sparkles (0x293E-0x296E) are gone.
    let gone: Vec<u16> = a.states.keys().filter(|k| !b.states.contains_key(k)).copied().collect();
    assert_eq!(gone, (0x28BE..=0x296E).step_by(16).collect::<Vec<u16>>());
    // Records that differ: the saucer's shot (CD_RELEASE.md §2), and two
    // actions the CD no longer calls.
    let differ: Vec<u16> =
        b.states.iter().filter(|(k, s)| a.states[*k] != **s).map(|(k, _)| *k).collect();
    assert_eq!(differ, [0x1B8E, 0x1B9E, 0x1BAE, 0x1D1E, 0x22FE]);
    assert_eq!((b.state(0x1B8E).unwrap().tics, b.state(0x1B9E).unwrap().shape), (6, 369));
    assert_eq!(a.state(0x22FE).unwrap().action, Some(Func::AManholeCover));
    assert_eq!(b.state(0x22FE).unwrap().action, None);
    // Same tables otherwise, except static 71's blocking flag.
    assert_eq!(a.attackinfo, b.attackinfo);
    assert_eq!(
        (a.levels.clone(), &a.pan_left, &a.pan_right),
        (b.levels.clone(), &b.pan_left, &b.pan_right)
    );
    let sdiff: Vec<usize> = (0..64).filter(|&i| a.statinfo[i] != b.statinfo[i]).collect();
    assert_eq!(sdiff, [48]);
    assert_eq!((a.statinfo[48].kind, b.statinfo[48].kind), (0, 17));
}
