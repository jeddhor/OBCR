//! Data-gated: the ally constants in `obc_sim::allies` match `COUNT.EXE`.

use obc_formats::DataDir;
use obc_formats::exe::MzExe;
use obc_sim::allies::{
    ALLYDEATHSNDS, S_ALLYCHASE1, S_ALLYDEAD, S_ALLYDIE1, S_ALLYPAIN1, S_ALLYPAIN2, S_ALLYSHOOT1,
    S_ALLYSTAY1,
};
use obc_sim::data::{DGROUP_FILE, Func, GameData};

fn load() -> Option<(MzExe, GameData)> {
    let dir = obc_testdata::data_dir()?;
    let exe = MzExe::parse(DataDir::open(dir).ok()?.read("COUNT.EXE").ok()?).ok()?;
    let data = GameData::load(&exe).ok()?;
    Some((exe, data))
}

fn ds_words(exe: &MzExe, off: usize, n: usize) -> Vec<i16> {
    let b = exe.file(DGROUP_FILE + off, n * 2).expect("DGROUP read");
    b.chunks_exact(2).map(|c| i16::from_le_bytes([c[0], c[1]])).collect()
}

fn ds_string(exe: &MzExe, off: usize, len: usize) -> String {
    let b = exe.file(DGROUP_FILE + off, len).expect("DGROUP read");
    String::from_utf8_lossy(b).into_owned()
}

#[test]
fn ally_state_chain_matches_count_exe() {
    let Some((_, data)) = load() else {
        eprintln!("SKIPPED: original game data not found (set OBC_DATA_DIR)");
        return;
    };
    let st = |id| *data.state(id).expect("ally state");

    let chase1 = st(S_ALLYCHASE1);
    assert_eq!(chase1.think, Some(Func::TAlly));
    assert_eq!(chase1.tics, 8);
    // Chase cycle: think states alternate with no-think states, back to chase1.
    let mut s = S_ALLYCHASE1;
    for i in 0..6 {
        let rec = st(s);
        assert_eq!(rec.think.is_some(), i % 2 == 0, "state {s:04x}");
        s = rec.next;
    }
    assert_eq!(s, S_ALLYCHASE1);

    let shoot1 = st(S_ALLYSHOOT1);
    assert_eq!(shoot1.action, Some(Func::AAlly));
    assert_eq!(shoot1.tics, 6);
    assert_eq!(st(shoot1.next).next, S_ALLYCHASE1);

    let stay1 = st(S_ALLYSTAY1);
    let stay2 = st(stay1.next);
    assert_eq!(stay1.think, None);
    assert_eq!(stay2.think, Some(Func::TAlly));
    assert_eq!(stay2.next, S_ALLYSTAY1);

    assert_eq!(st(S_ALLYPAIN1).next, S_ALLYPAIN2);
    assert_eq!(st(S_ALLYPAIN2).next, S_ALLYCHASE1);

    let die1 = st(S_ALLYDIE1);
    assert_eq!(die1.action, Some(Func::ADeathScream));
    let mut s = S_ALLYDIE1;
    for _ in 0..7 {
        s = st(s).next;
    }
    assert_eq!(s, S_ALLYDEAD);
    assert_eq!(st(S_ALLYDEAD).next, S_ALLYDEAD);
    assert_eq!(st(S_ALLYDEAD).tics, 0);
}

#[test]
fn ally_tables_and_strings_match_count_exe() {
    let Some((exe, _)) = load() else {
        eprintln!("SKIPPED: original game data not found (set OBC_DATA_DIR)");
        return;
    };
    // Tables transcribed in allies.rs.
    assert_eq!(ds_words(&exe, 0x32EC, 8), [400, 400, 400, 400, 800, 800, 800, 400]); // allyhp
    assert_eq!(ds_words(&exe, 0x2BCA, 3), [47, 45, 46]); // allyfiresnds
    assert_eq!(ds_words(&exe, 0x0F22, 6), [54, 55, 56, 53, 53, 51]); // sighting sounds
    let screams: Vec<i16> = ALLYDEATHSNDS.iter().map(|&s| s as i16).collect();
    assert_eq!(ds_words(&exe, 0x29A0, 5), screams);
    assert_eq!(ds_words(&exe, 0x0E68, 9), [4, 5, 6, 7, 0, 1, 2, 3, 8]); // opposite
    assert_eq!(ds_words(&exe, 0x0004, 1), [0x6F42]); // read by T_Ally's NULL target
    assert_eq!(ds_words(&exe, 0x0D1A, 2), [0, 1]); // TILEGLOBAL as a long

    // GameMessage strings.
    for (off, text) in [
        (0x2BD2, "DISPERSE AND ATTACK\0"),
        (0x2BE6, "FOLLOW PLAYER\0"),
        (0x0ADF, "SWITCH\0"),
        (0x0AE6, "NO ALLIES!\0"),
        (0x0AF1, "STAYPUT & SWITCH\0"),
    ] {
        assert_eq!(ds_string(&exe, off, text.len()), text);
    }
}
