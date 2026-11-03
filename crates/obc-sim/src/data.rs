//! Behavior tables read from the user's `COUNT.EXE` at startup (ADR-005):
//! actor state records, the player attack table and static-object info.
//!
//! Function pointers in state records are far pointers into code segments;
//! they are resolved to [`Func`] by their Ghidra address (docs/re/ACTORS.md §11-12).

use std::collections::BTreeMap;

use obc_formats::Result;
use obc_formats::exe::MzExe;

/// File offset of DGROUP (DS:0000) in `COUNT.EXE` (release OBC-A).
pub const DGROUP_FILE: usize = 0x35580;

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
    /// Maps a Ghidra address (`segment << 16 | offset`) to a function.
    pub fn from_addr(addr: u32) -> Self {
        match addr {
            0x2390_1d1d => Func::TStand,
            0x2390_2047 => Func::TPath,
            0x2390_1d2d => Func::TChase,
            0x2390_179b => Func::TBossChase,
            0x2390_1966 => Func::TFatChase,
            0x2390_1b31 => Func::TSwampChase,
            0x2390_0125 => Func::TProjectile,
            0x2390_1620 => Func::TFireBurnout,
            0x2390_28d1 => Func::TBossDead,
            0x208c_1bfb => Func::TPlayer,
            0x208c_18dc => Func::TAttack,
            0x2634_07b8 => Func::TAlly,
            0x2390_2321 => Func::AShoot,
            0x2390_2506 => Func::TShoot,
            0x2390_21b7 => Func::AIdleSound,
            0x2390_0c59 => Func::ADeathScream,
            0x2390_0c4a => Func::ARemove,
            0x2390_2a49 => Func::ANop,
            0x2390_1508 => Func::ASlimeThrow,
            0x2390_1b17 => Func::AManholeCover,
            0x2390_1274 => Func::ADebugSparkle,
            0x2634_0214 => Func::AAlly,
            a => Func::Unknown(a),
        }
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

/// All tables the simulation reads from the executable.
#[derive(Debug, Clone)]
pub struct GameData {
    pub states: BTreeMap<StateId, State>,
    /// `attackinfo[weapon][step]` (DS:0x0F66).
    pub attackinfo: [[AttackStep; 15]; 5],
    /// `statinfo` (DS:0x11DA).
    pub statinfo: Vec<StatInfo>,
}

/// State-record ranges (DS offsets, inclusive): player, enemies/effects, allies.
const STATE_RANGES: [(u16, u16); 3] = [(0x0F46, 0x0F56), (0x12EE, 0x296E), (0x2A26, 0x2BB6)];

fn ds(exe: &MzExe, off: u16, len: usize) -> Result<&[u8]> {
    exe.file(DGROUP_FILE + off as usize, len)
}

fn far_ptr(bytes: &[u8]) -> Option<Func> {
    let off = u16::from_le_bytes([bytes[0], bytes[1]]);
    let seg = u16::from_le_bytes([bytes[2], bytes[3]]);
    if off == 0 && seg == 0 {
        return None;
    }
    // Stored segments are image-relative; Ghidra's base adds 0x1000.
    Some(Func::from_addr((u32::from(seg) + 0x1000) << 16 | u32::from(off)))
}

impl GameData {
    pub fn load(exe: &MzExe) -> Result<Self> {
        let mut states = BTreeMap::new();
        for (first, last) in STATE_RANGES {
            let mut off = first;
            while off <= last {
                let r = ds(exe, off, 16)?;
                let w = |i: usize| i16::from_le_bytes([r[i], r[i + 1]]);
                states.insert(
                    off,
                    State {
                        rotate: w(0) as u16,
                        shape: w(2),
                        tics: w(4),
                        think: far_ptr(&r[6..10]),
                        action: far_ptr(&r[10..14]),
                        next: w(14) as u16,
                    },
                );
                off += 16;
            }
        }
        let raw = ds(exe, 0x0F66, 5 * 15 * 4)?;
        let mut attackinfo = [[AttackStep::default(); 15]; 5];
        for (w, row) in attackinfo.iter_mut().enumerate() {
            for (k, step) in row.iter_mut().enumerate() {
                let b = &raw[(w * 15 + k) * 4..];
                *step = AttackStep { tics: b[0] as i8, attack: b[1] as i8, frame: b[2] as i8 };
            }
        }
        // statinfo runs until the attackinfo-adjacent tables; 63 entries cover codes 23..85.
        let raw = ds(exe, 0x11DA, 64 * 4)?;
        let statinfo = raw
            .chunks_exact(4)
            .map(|c| StatInfo {
                shape: i16::from_le_bytes([c[0], c[1]]),
                kind: i16::from_le_bytes([c[2], c[3]]),
            })
            .collect();
        Ok(Self { states, attackinfo, statinfo })
    }

    pub fn state(&self, id: StateId) -> Option<&State> {
        self.states.get(&id)
    }
}

/// Player states (DS offsets).
pub const S_PLAYER: StateId = 0x0F46;
pub const S_ATTACK: StateId = 0x0F56;
