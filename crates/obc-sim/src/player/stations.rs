//! Healing stations (OBC, no Wolf3D counterpart; docs/re/PLAYER.md §9).
//!
//! `COUNT.EXE` holds a complete healing-station mechanism: a table of up to
//! 15 stations with a health pool each (*SpawnHealStation* ghidra 2f4d:064a),
//! a routine that points the player at one (*StartHealStation* 2f4d:06c9) and
//! a per-frame routine (*HealStations* 2f4d:06ec) that turns the player
//! toward the station, closes an animated wall two tiles ahead and then heals
//! `tics` points per frame from the pool. In release OBC-A only the per-frame
//! routine is reachable (PlayLoop calls it every frame, 1942:15f1): the
//! spawner, the starter and the flag setter (2f4d:041f) have no callers and
//! no stored pointers, so the station flags are never set and the routine
//! returns at once (CONFIRMED by an exhaustive scan of the image for far and
//! near calls and far pointers to them; PLAYER.md §9).
//!
//! The whole mechanism is ported anyway so that the per-frame call stays in
//! the frame order and a future release (or a modded map) behaves as the
//! code says.

use super::heal_self;
use crate::events::SimEvent;
use crate::world::World;

/// `gamestate + 0x16` bit: a station is healing the player.
pub const HEALING: u16 = 0x10;
/// `gamestate + 0x16` bit: the player is turning toward a station.
pub const TURNING: u16 = 0x40;

/// Station table size (DS:0x4862, 16 × 8 bytes). *SpawnHealStation* refuses
/// when `count + 1 >= 16`, so at most 15 are used.
pub const MAXSTATIONS: usize = 16;

/// Health pool by station kind 0–3 (jump table 2f4d:06c1, CONFIRMED).
pub const STATION_POOL: [i16; 4] = [50, 100, 200, 300];

/// Angle to face by direction 0–3 (DS:0x326A = 270, 180, 90, 0, CONFIRMED file read).
pub const STATION_ANGLE: [i16; 4] = [270, 180, 90, 0];

/// One station record (8 bytes at DS:0x4862 + 8·i).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct HealStation {
    /// +0, +2: tile (written by the spawner, read by nothing).
    pub x: i16,
    pub y: i16,
    /// +4: health left in the station.
    pub pool: i16,
    /// +6: `tics << 5`, written every active frame; no reader (UNKNOWN purpose).
    pub rate: i16,
}

/// Station globals.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Stations {
    /// The table (DS:0x4862; saved by the original's `SaveTheGame`).
    pub list: [HealStation; MAXSTATIONS],
    /// Stations spawned this level (DS:0xE7B2, cleared by `PrepareLevel`).
    pub count: i16,
    /// Angle the player turns to (DS:0xDD89).
    pub angle: i16,
    /// Tile whose animated wall is closed once the player faces the station
    /// (DS:0xDD8D x, DS:0xDD8B y); moved two tiles along the facing angle.
    pub wall_x: i16,
    pub wall_y: i16,
}

/// *SpawnHealStation*(x, y, kind) (ghidra 2f4d:064a, CONFIRMED; no caller in
/// OBC-A). Kinds 0–3 get a pool of 50, 100, 200 or 300; other kinds keep
/// whatever the record held.
#[doc(alias = "SpawnHealStation")]
pub fn spawn_heal_station(w: &mut World, x: i16, y: i16, kind: u16) {
    let st = &mut w.ply.stations;
    if st.count.wrapping_add(1) >= MAXSTATIONS as i16 {
        return;
    }
    let Some(rec) = st.list.get_mut(st.count as usize) else { return };
    rec.x = x;
    rec.y = y;
    if let Some(&pool) = STATION_POOL.get(usize::from(kind)) {
        rec.pool = pool;
    }
    st.count += 1;
}

/// *StartHealStation*(dir, station, x, y) (ghidra 2f4d:06c9, CONFIRMED; no
/// caller in OBC-A): records the station, the angle to face and the tile the
/// wall test starts from. It does **not** set [`TURNING`]; the original
/// leaves that to the flag setter 2f4d:041f (`gamestate+0x16 |= mask`, also
/// uncalled).
#[doc(alias = "StartHealStation")]
pub fn start_heal_station(w: &mut World, dir: u16, station: i16, x: i16, y: i16) {
    // DS:0x326A has four entries; larger `dir` values read the next words.
    let Some(&angle) = STATION_ANGLE.get(usize::from(dir)) else {
        w.emit(SimEvent::Fault(format!("StartHealStation: direction {dir} out of range")));
        return;
    };
    w.gamestate.station = station;
    let st = &mut w.ply.stations;
    st.angle = angle;
    st.wall_x = x;
    st.wall_y = y;
}

/// *HealStations* (ghidra 2f4d:06ec, CONFIRMED), once per frame after
/// `MoveAnimWalls`.
///
/// While [`TURNING`]: the player's angle moves `tics` degrees toward the
/// target (plain signed comparison, no wrap-around), and when it matches,
/// [`TURNING`] becomes [`HEALING`], the wall tile moves two tiles along the
/// facing angle and every open (state 0) animated wall record on that tile
/// is operated (closes it). While [`HEALING`]: if the pool is above 0 and
/// health is below `maxhealth`, `HealSelf(tics)` and the pool drops by
/// `tics`; otherwise [`HEALING`] is cleared. No `US_RndT` calls.
#[doc(alias = "HealStations")]
pub fn heal_stations(w: &mut World) {
    let flags = w.gamestate.itemflags;
    if flags & (TURNING | HEALING) == 0 {
        return;
    }
    let tics = w.tics;
    let i = w.gamestate.station;
    // The original indexes DS:0x4866 + 8·i unchecked.
    if !(0..MAXSTATIONS as i16).contains(&i) {
        w.emit(SimEvent::Fault(format!("HealStations: station {i} out of range")));
        return;
    }
    let i = i as usize;
    w.ply.stations.list[i].rate = tics.wrapping_shl(5);

    if flags & TURNING != 0 {
        let target = w.ply.stations.angle;
        let p = w.player;
        let o = w.obj_mut(p);
        if o.angle > target {
            o.angle = o.angle.wrapping_sub(tics);
            if o.angle < target {
                o.angle = target;
            }
        } else if o.angle < target {
            o.angle = o.angle.wrapping_add(tics);
            if o.angle > target {
                o.angle = target;
            }
        }
        if o.angle != target {
            return;
        }
        w.gamestate.itemflags = (w.gamestate.itemflags & !TURNING) | HEALING;
        let st = &mut w.ply.stations;
        match target {
            180 => st.wall_x = st.wall_x.wrapping_sub(2),
            0 => st.wall_x = st.wall_x.wrapping_add(2),
            90 => st.wall_y = st.wall_y.wrapping_sub(2),
            270 => st.wall_y = st.wall_y.wrapping_add(2),
            _ => {}
        }
        let (x, y) = (st.wall_x, st.wall_y);
        for k in 0..w.animwalls.len() {
            let a = w.animwalls[k];
            if a.x as i16 == x && a.y as i16 == y && a.state == 0 {
                w.operate_anim_wall(i32::from(x), i32::from(y));
            }
        }
        return;
    }

    if w.ply.stations.list[i].pool > 0 && w.gamestate.health < w.gamestate.maxhealth {
        heal_self(w, tics);
        let rec = &mut w.ply.stations.list[i];
        rec.pool = rec.pool.wrapping_sub(tics);
        return;
    }
    w.gamestate.itemflags &= !HEALING;
}
