//! Data-gated: every state record resolves to known functions.

use obc_formats::DataDir;
use obc_formats::exe::MzExe;
use obc_sim::data::{Func, GameData};

#[test]
fn state_tables_resolve() {
    let Some(dir) = obc_testdata::data_dir() else {
        eprintln!("SKIPPED: original game data not found (set OBC_DATA_DIR)");
        return;
    };
    let exe = MzExe::parse(DataDir::open(dir).unwrap().read("COUNT.EXE").unwrap()).unwrap();
    let data = GameData::load(&exe).unwrap();
    let mut unknown = std::collections::BTreeSet::new();
    for (id, s) in &data.states {
        for f in [s.think, s.action].into_iter().flatten() {
            if let Func::Unknown(a) = f {
                unknown.insert((format!("{a:08x}"), *id));
            }
        }
        assert!(
            s.next == 0 || data.states.contains_key(&s.next),
            "state {id:04x} -> {:04x}",
            s.next
        );
    }
    for (a, id) in &unknown {
        eprintln!("unknown function {a} in state {id:04x}");
    }
    assert_eq!(data.state(0x14AE).unwrap().shape, 64, "rat stand sprite");
    assert_eq!(data.attackinfo[0][3].tics, 56, "pistol raised frame");
}
