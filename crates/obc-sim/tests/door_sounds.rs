//! Door sounds (issue #15): every door plays sound 0x52 (82) as it starts to
//! open (`DoorOpening` 2281:0868) and as it closes (`CloseDoor` 2281:0528),
//! at the door's tile (`PlaySoundLocTile`). A synthetic map: no game data.

use std::sync::Arc;

use obc_core::trig::Tables;
use obc_sim::data::GameData;
use obc_sim::events::SimEvent;
use obc_sim::world::{AREATILE, Door, DoorAction, MAPSIZE, Occ, World};

const WALL: u8 = 22;

/// A walled room in area 0 with a vertical door at (10, 10) of `lock`, the
/// player at (5, 5).
fn world(lock: u8) -> World {
    let mut w = World::new(Arc::new(GameData::default()), Arc::new(Tables::build()));
    for x in 0..MAPSIZE {
        for y in 0..MAPSIZE {
            let edge = x == 0 || y == 0 || x == MAPSIZE - 1 || y == MAPSIZE - 1;
            if edge {
                w.tilemap[x * MAPSIZE + y] = WALL;
                w.actorat[x * MAPSIZE + y] = Occ::Mark(u16::from(WALL));
                w.plane0[y * MAPSIZE + x] = u16::from(WALL);
            } else {
                w.plane0[y * MAPSIZE + x] = AREATILE;
            }
        }
    }
    w.doors.push(Door {
        tilex: 10,
        tiley: 10,
        vertical: true,
        lock,
        action: DoorAction::Closed,
        ticcount: 0,
        kind: 128,
        position: 0,
    });
    w.tilemap[10 * MAPSIZE + 10] = 0x80;
    w.actorat[10 * MAPSIZE + 10] = Occ::Mark(0x80);
    let p = w.player;
    let o = w.obj_mut(p);
    o.tilex = 5;
    o.tiley = 5;
    o.x = (5 << 16) + 0x8000;
    o.y = (5 << 16) + 0x8000;
    o.areanumber = 0;
    w.connect_areas();
    w.tics = 1;
    w
}

fn sounds(w: &mut World) -> Vec<(u16, i32, i32)> {
    std::mem::take(&mut w.events)
        .into_iter()
        .filter_map(|e| match e {
            SimEvent::SoundAt { id, x, y } => Some((id, x, y)),
            _ => None,
        })
        .collect()
}

#[test]
fn every_door_kind_opens_and_closes_with_sound_82() {
    let centre = (10 << 16) + 0x8000;
    // Locks 0-5 cover the plain, keyed and elevator door kinds.
    for lock in 0..6 {
        let mut w = world(lock);
        w.open_door(0);
        w.move_doors();
        assert_eq!(sounds(&mut w), vec![(82, centre, centre)], "lock {lock}: opening");
        for _ in 0..200 {
            w.move_doors();
        }
        assert_eq!(w.doors[0].action, DoorAction::Open, "lock {lock}");
        sounds(&mut w);
        w.close_door(0);
        assert_eq!(sounds(&mut w), vec![(82, centre, centre)], "lock {lock}: closing");
    }
}
