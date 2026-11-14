//! Behavior tables read from the user's executable at startup (ADR-005):
//! actor state records, the player attack table and static-object info.
//!
//! Function pointers in state records are far pointers into code segments;
//! they are resolved to [`Func`] by their Ghidra address (docs/re/ACTORS.md §11-12).
//!
//! The CD release's `BCCD.EXE` holds the same tables at other offsets
//! ([`ExeLayout`], docs/re/CD_RELEASE.md §2). State records keep OBC-A's DS
//! offsets as their identity in both releases: the CD's are translated when
//! loaded, so the simulation names states one way.

use std::collections::BTreeMap;

use obc_formats::Result;
use obc_formats::exe::MzExe;

/// File offset of DGROUP (DS:0000) in `COUNT.EXE` (release OBC-A).
pub const DGROUP_FILE: usize = 0x35580;

/// Where one release's executable keeps the tables [`GameData`] reads.
#[derive(Debug)]
pub struct ExeLayout {
    /// File offset of DGROUP (DS:0000).
    pub dgroup_file: usize,
    pub pan_right: u16,
    pub pan_left: u16,
    pub songs: u16,
    pub ramps: u16,
    /// `Rindex`; `Findex`, `Rclr` and `Fclr` follow at +51 bytes each.
    pub rindex: u16,
    pub attackinfo: u16,
    pub statinfo: u16,
    pub floor_sets: u16,
    /// State-record ranges `(first, last, OBC-A first)`, DS offsets inclusive.
    pub states: [(u16, u16, u16); 3],
    /// Far-pointer targets (Ghidra addresses) of the functions states use.
    pub funcs: &'static [(u32, Func)],
    /// True for `BCCD.EXE`, whose code differs in places (CD_RELEASE.md §3).
    pub cd: bool,
}

/// `COUNT.EXE`, release OBC-A.
pub static OBC_A: ExeLayout = ExeLayout {
    dgroup_file: DGROUP_FILE,
    pan_right: 0x0594,
    pan_left: 0x0756,
    songs: 0x098A,
    ramps: 0x09A4,
    rindex: 0x09E6,
    attackinfo: 0x0F66,
    statinfo: 0x11DA,
    floor_sets: 0x33BE,
    states: [(0x0F46, 0x0F56, 0x0F46), (0x12EE, 0x296E, 0x12EE), (0x2A26, 0x2BB6, 0x2A26)],
    funcs: &[
        (0x2390_1d1d, Func::TStand),
        (0x2390_2047, Func::TPath),
        (0x2390_1d2d, Func::TChase),
        (0x2390_179b, Func::TBossChase),
        (0x2390_1966, Func::TFatChase),
        (0x2390_1b31, Func::TSwampChase),
        (0x2390_0125, Func::TProjectile),
        (0x2390_1620, Func::TFireBurnout),
        (0x2390_28d1, Func::TBossDead),
        (0x208c_1bfb, Func::TPlayer),
        (0x208c_18dc, Func::TAttack),
        (0x2634_07b8, Func::TAlly),
        (0x2390_2321, Func::AShoot),
        (0x2390_2506, Func::TShoot),
        (0x2390_21b7, Func::AIdleSound),
        (0x2390_0c59, Func::ADeathScream),
        (0x2390_0c4a, Func::ARemove),
        (0x2390_2a49, Func::ANop),
        (0x2390_1508, Func::ASlimeThrow),
        (0x2390_1b17, Func::AManholeCover),
        (0x2390_1274, Func::ADebugSparkle),
        (0x2634_0214, Func::AAlly),
    ],
    cd: false,
};

/// `BCCD.EXE`, release OBC-CD. Tables found by content and state records
/// aligned one-to-one with OBC-A's (CD_RELEASE.md §2): the CD drops the 12
/// face-apparition and sparkle states at the end of the second range, so
/// the allies' range starts 186 bytes earlier. Function addresses come from
/// pairing the aligned records' far pointers.
pub static OBC_CD: ExeLayout = ExeLayout {
    dgroup_file: 0x35C20,
    pan_right: 0x056C,
    pan_left: 0x072E,
    songs: 0x0962,
    ramps: 0x097C,
    rindex: 0x09BE,
    attackinfo: 0x0F6C,
    statinfo: 0x11E0,
    floor_sets: 0x32D8,
    states: [(0x0F4C, 0x0F5C, 0x0F46), (0x12F4, 0x28B4, 0x12EE), (0x296C, 0x2AFC, 0x2A26)],
    funcs: &[
        (0x23b5_19d8, Func::TStand),
        (0x23b5_1d01, Func::TPath),
        (0x23b5_19e8, Func::TChase),
        (0x23b5_1470, Func::TBossChase),
        (0x23b5_163b, Func::TFatChase),
        (0x23b5_17ec, Func::TSwampChase),
        (0x23b5_0121, Func::TProjectile),
        (0x23b5_12f5, Func::TFireBurnout),
        (0x23b5_2567, Func::TBossDead),
        (0x20b2_1be7, Func::TPlayer),
        (0x20b2_18c8, Func::TAttack),
        (0x2623_07ae, Func::TAlly),
        (0x23b5_1fdb, Func::AShoot),
        (0x23b5_21c0, Func::TShoot),
        (0x23b5_1e71, Func::AIdleSound),
        (0x23b5_0c1b, Func::ADeathScream),
        (0x23b5_0c0c, Func::ARemove),
        (0x23b5_26df, Func::ANop),
        (0x23b5_11dd, Func::ASlimeThrow),
        (0x2623_020a, Func::AAlly),
    ],
    cd: true,
};

impl ExeLayout {
    /// The layout for a release id (`obc_formats::release`); OBC-A otherwise.
    pub fn for_release(id: &str) -> &'static ExeLayout {
        if id == "OBC-CD" { &OBC_CD } else { &OBC_A }
    }

    /// OBC-A's DS offset for a state record at `ds` in this executable.
    pub fn canonical_state(&self, ds: u16) -> StateId {
        for (first, last, canon) in self.states {
            if (first..=last).contains(&ds) {
                return ds - first + canon;
            }
        }
        ds
    }

    fn func(&self, addr: u32) -> Func {
        self.funcs.iter().find(|(a, _)| *a == addr).map_or(Func::Unknown(addr), |&(_, f)| f)
    }
}

/// A state record's identity: its DS offset (`statetype *`). 0 is NULL.
pub type StateId = u16;

/// Functions referenced by state records and other tables.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Func {
    TStand,
    TPath,
    TChase,
    TBossChase,
    TFatChase,
    TSwampChase,
    TProjectile,
    TFireBurnout,
    TBossDead,
    TPlayer,
    TAttack,
    TAlly,
    AShoot,
    TShoot,
    AIdleSound,
    ADeathScream,
    ARemove,
    ANop,
    ASlimeThrow,
    AManholeCover,
    ADebugSparkle,
    AAlly,
    Unknown(u32),
}

impl Func {
    /// Maps an OBC-A Ghidra address (`segment << 16 | offset`) to a function.
    pub fn from_addr(addr: u32) -> Self {
        OBC_A.func(addr)
    }
}

/// A `statetype` record.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct State {
    pub rotate: u16,
    pub shape: i16,
    pub tics: i16,
    pub think: Option<Func>,
    pub action: Option<Func>,
    pub next: StateId,
}

/// One `attackinfo` entry: `{tics, attack, frame}`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct AttackStep {
    pub tics: i8,
    pub attack: i8,
    pub frame: i8,
}

/// `statinfo` entry: sprite and item type of a static object.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StatInfo {
    pub shape: i16,
    pub kind: i16,
}

/// Per-level presentation tables (docs/re/LEVEL_CODES.md §5).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct LevelTables {
    /// Roof ramp index, floor ramp index, roof colour, floor colour per map slot 0..51
    /// (`Rindex`/`Findex`/`Rclr`/`Fclr`, DS:0x9E6/0xA19/0xA4C/0xA7F).
    pub rindex: Vec<u8>,
    pub findex: Vec<u8>,
    pub rclr: Vec<u8>,
    pub fclr: Vec<u8>,
    /// Ramps `{first, last}` (DS:0x9A4).
    pub ramps: Vec<(u8, u8)>,
    /// Floor/ceiling texture sets: `(floor page + 1, ceiling page + 1)` (DS:0x33BE).
    pub floor_sets: [(u8, u8); 10],
    /// Song per map for maps 0..9 (DS:0x098A).
    pub songs: [u16; 10],
}

impl LevelTables {
    /// Table index for a map slot: net levels 50-59 reuse entries 40-49.
    pub fn index(mapon: i16) -> usize {
        if mapon < 50 { mapon.max(0) as usize } else { (mapon - 10).clamp(0, 50) as usize }
    }
}

/// All tables the simulation reads from the executable.
#[derive(Debug, Clone, Default)]
pub struct GameData {
    pub levels: LevelTables,
    pub states: BTreeMap<StateId, State>,
    /// `attackinfo[weapon][step]` (DS:0x0F66).
    pub attackinfo: [[AttackStep; 15]; 5],
    /// `statinfo` (DS:0x11DA).
    pub statinfo: Vec<StatInfo>,
    /// Positioned-sound attenuation, `[x][y + 15]` with 15 × 30 entries, 0 (loud)
    /// to 7: `righttable` DS:0x0594 and `lefttable` DS:0x0756. OBC's values
    /// differ from Wolf3D's (`WOLFSRC/WL_GAME.C:77`, maximum 8).
    pub pan_right: Vec<u8>,
    pub pan_left: Vec<u8>,
    /// Loaded from `BCCD.EXE` (CD_RELEASE.md §3 lists where its code differs).
    pub cd: bool,
}

fn far_ptr(layout: &ExeLayout, bytes: &[u8]) -> Option<Func> {
    let off = u16::from_le_bytes([bytes[0], bytes[1]]);
    let seg = u16::from_le_bytes([bytes[2], bytes[3]]);
    if off == 0 && seg == 0 {
        return None;
    }
    // Stored segments are image-relative; Ghidra's base adds 0x1000.
    Some(layout.func((u32::from(seg) + 0x1000) << 16 | u32::from(off)))
}

impl GameData {
    /// Reads `COUNT.EXE` (release OBC-A).
    pub fn load(exe: &MzExe) -> Result<Self> {
        Self::load_with(exe, &OBC_A)
    }

    /// Reads the tables from `exe` laid out as `layout`.
    pub fn load_with(exe: &MzExe, layout: &ExeLayout) -> Result<Self> {
        let ds = |off: u16, len: usize| exe.file(layout.dgroup_file + off as usize, len);
        let mut states = BTreeMap::new();
        for (first, last, _) in layout.states {
            let mut off = first;
            while off <= last {
                let r = ds(off, 16)?;
                let w = |i: usize| i16::from_le_bytes([r[i], r[i + 1]]);
                states.insert(
                    layout.canonical_state(off),
                    State {
                        rotate: w(0) as u16,
                        shape: w(2),
                        tics: w(4),
                        think: far_ptr(layout, &r[6..10]),
                        action: far_ptr(layout, &r[10..14]),
                        next: layout.canonical_state(w(14) as u16),
                    },
                );
                off += 16;
            }
        }
        let raw = ds(layout.attackinfo, 5 * 15 * 4)?;
        let mut attackinfo = [[AttackStep::default(); 15]; 5];
        for (w, row) in attackinfo.iter_mut().enumerate() {
            for (k, step) in row.iter_mut().enumerate() {
                let b = &raw[(w * 15 + k) * 4..];
                *step = AttackStep { tics: b[0] as i8, attack: b[1] as i8, frame: b[2] as i8 };
            }
        }
        // statinfo runs until the attackinfo-adjacent tables; 63 entries cover codes 23..85.
        let raw = ds(layout.statinfo, 64 * 4)?;
        let statinfo = raw
            .chunks_exact(4)
            .map(|c| StatInfo {
                shape: i16::from_le_bytes([c[0], c[1]]),
                kind: i16::from_le_bytes([c[2], c[3]]),
            })
            .collect();
        let levels = LevelTables {
            rindex: ds(layout.rindex, 51)?.to_vec(),
            findex: ds(layout.rindex + 51, 51)?.to_vec(),
            rclr: ds(layout.rindex + 102, 51)?.to_vec(),
            fclr: ds(layout.rindex + 153, 51)?.to_vec(),
            ramps: ds(layout.ramps, 13 * 4)?.chunks_exact(4).map(|c| (c[0], c[2])).collect(),
            floor_sets: {
                let b = ds(layout.floor_sets, 20)?;
                std::array::from_fn(|i| (b[2 * i], b[2 * i + 1]))
            },
            songs: {
                let b = ds(layout.songs, 20)?;
                std::array::from_fn(|i| u16::from_le_bytes([b[2 * i], b[2 * i + 1]]))
            },
        };
        Ok(Self {
            levels,
            states,
            attackinfo,
            statinfo,
            pan_right: ds(layout.pan_right, 450)?.to_vec(),
            pan_left: ds(layout.pan_left, 450)?.to_vec(),
            cd: layout.cd,
        })
    }

    pub fn state(&self, id: StateId) -> Option<&State> {
        self.states.get(&id)
    }
}

/// Player states (DS offsets).
pub const S_PLAYER: StateId = 0x0F46;
pub const S_ATTACK: StateId = 0x0F56;
