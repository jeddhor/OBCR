//! Level setup: `SetupGameLevel` (ghidra 1823:0919), `PrepareLevel`
//! (2f4d:0167) and `ScanInfoPlane` (1823:01c8). Spec: docs/re/LEVEL_CODES.md §2-4.

use obc_formats::maps::Map;

use crate::actors;
use crate::allies;
use crate::player;
use crate::world::{AMBUSHTILE, AREATILE, Door, DoorAction, MAPSIZE, NUMAREAS, Occ, World, ex};

/// Plane-0 door codes (130–137) and the per-lock door page (`DOORWALL` = 128).
pub const DOOR_FIRST: u16 = 130;
pub const DOOR_LAST: u16 = 137;
/// Door page by lock (`HitHorizDoor` table ghidra 1b4a:0895).
pub const DOOR_PAGES: [u16; 6] = [128, 132, 133, 132, 133, 135];
/// Plane-1 push-wall marker.
pub const PUSHABLETILE: u16 = 98;

/// Sewer floor/ceiling overrides for sets 0–3 on maps 0–4 (ghidra 30d6:1c02),
/// as `(floor page + 1, ceiling page + 1)`.
const SEWER_FLOOR_SETS: [(u8, u8); 4] = [(12, 12), (106, 108), (110, 112), (12, 108)];

impl World {
    /// `SetupGameLevel` for `map` as level `mapon`.
    pub fn setup_game_level(&mut self, map: &Map, mapon: i16) {
        if !self.loadedgame {
            let g = &mut self.gamestate;
            g.killcount = 0;
            g.secretcount = 0;
            g.killtotal = 0;
            g.secrettotal = 0;
            g.timecount = 0;
        }
        self.gamestate.mapon = mapon;
        self.plane0 = map.planes[0].clone();
        self.plane1 = map.planes[1].clone();
        self.tilemap.fill(0);
        self.actorat.fill(Occ::Empty);
        self.spotvis.fill(false);

        // Plane-0 pass: solid cells below AREATILE; code 143 flags plane 1 bit 15.
        for y in 0..MAPSIZE as i32 {
            for x in 0..MAPSIZE as i32 {
                let tile = self.mapspot(x, y, 0);
                let i = Self::idx(x, y);
                if tile < AREATILE {
                    self.tilemap[i] = tile as u8;
                    self.actorat[i] = Occ::Mark(tile);
                } else if tile == 143 {
                    let v = self.mapspot(x, y, 1) | 0x8000;
                    self.set_mapspot(x, y, 1, v);
                }
            }
        }

        self.init_actor_list();
        self.doors.clear();
        self.areaconnect = [[0; NUMAREAS]; NUMAREAS];
        self.areabyplayer = [false; NUMAREAS];
        self.statics.clear();
        self.animwalls.clear();
        self.pwall = Default::default();

        // Doors: even codes vertical, odd horizontal; lock = (code - 130) / 2.
        for y in 0..MAPSIZE as i32 {
            for x in 0..MAPSIZE as i32 {
                let tile = self.mapspot(x, y, 0);
                if (DOOR_FIRST..=DOOR_LAST).contains(&tile) {
                    let off = tile - DOOR_FIRST;
                    self.spawn_door(x, y, off.is_multiple_of(2), (off / 2) as u8);
                }
            }
        }

        self.prepare_level();
        // `ingame` (DS:0x8CBC) is clear while the map is scanned, so fires placed
        // on the map make no sound and draw no random number.
        self.act.ingame = false;
        self.scan_info_plane();
        self.act.ingame = true;

        // Ambush cells (138) become floor with a neighbouring area code.
        for y in 0..MAPSIZE as i32 {
            for x in 0..MAPSIZE as i32 {
                let i = Self::idx(x, y);
                let is_ambush =
                    self.mapspot(x, y, 0) == AMBUSHTILE || self.actorat[i] == Occ::Mark(AMBUSHTILE);
                if !is_ambush || self.mapspot(x, y, 1) & 0x7FFF < 108 {
                    continue;
                }
                self.tilemap[i] = 0;
                if self.actorat[i] == Occ::Mark(AMBUSHTILE) {
                    self.actorat[i] = Occ::Empty;
                }
                let mut code = self.mapspot(x, y, 0);
                for (dx, dy) in [(1, 0), (0, -1), (0, 1), (-1, 0)] {
                    let t = self.mapspot(x + dx, y + dy, 0);
                    if t >= AREATILE {
                        code = t;
                    }
                }
                self.set_mapspot(x, y, 0, code);
            }
        }
        self.level_floor_sets(mapon);
        // Sewers are dark (PlayLoop sets DS:0x33B8 = mapon <= 4).
        self.dark = (0..=4).contains(&mapon);
        self.quake = 0;
        self.shake = 0;
        self.playstate = crate::world::ex::STILLPLAYING;
    }

    /// `SpawnDoor` (ghidra 2281:0437): closed, solid, area copied from the west/north neighbour.
    fn spawn_door(&mut self, x: i32, y: i32, vertical: bool, lock: u8) {
        if self.doors.len() >= crate::doors::MAXDOORS {
            return;
        }
        let n = self.doors.len() as u16;
        self.doors.push(Door {
            tilex: x as u8,
            tiley: y as u8,
            vertical,
            lock,
            action: DoorAction::Closed,
            ticcount: 0,
            kind: DOOR_PAGES[lock as usize % DOOR_PAGES.len()],
            position: 0,
        });
        let i = Self::idx(x, y);
        self.tilemap[i] = (n | 0x80) as u8;
        self.actorat[i] = Occ::Mark(n | 0x80);
        let area = if vertical { self.mapspot(x - 1, y, 0) } else { self.mapspot(x, y - 1, 0) };
        self.set_mapspot(x, y, 0, area);
    }

    /// `PrepareLevel` (ghidra 2f4d:0167): per-level resets.
    fn prepare_level(&mut self) {
        // DS:0xE7A8 (animated walls) is cleared by setup_game_level; DS:0xE7B2
        // is the healing-station count.
        self.ply.stations.count = 0;
        self.actorpos = [0; crate::world::MAXACTORS];
        player::setup_level_player_state(self);
        // The overhead-map wall bitmap (DS:0xE093, 2f4d:0200; RENDERER.md §9.2):
        // a solid wall 1–0x7F unless plane 1 is 107, or an open/door cell whose
        // plane-1 bit 15 is set (plane-0 code 143). Then bit 15 is cleared.
        for y in 0..MAPSIZE as i32 {
            let mut row = 0u64;
            for x in 0..MAPSIZE as i32 {
                let t = self.tile(x, y);
                let info = self.mapspot(x, y, 1);
                let wall = (1..=0x7F).contains(&t) && info != 107;
                let marked = (t == 0 || t & 0x80 != 0) && info & 0x8000 != 0;
                if wall || marked {
                    row |= 1 << x;
                }
            }
            self.automap[y as usize] = row;
        }
        for v in &mut self.plane1 {
            *v &= 0x7FFF;
        }
    }

    /// `ScanInfoPlane`: plane-1 codes 19–317, row-major (RNG order matters).
    fn scan_info_plane(&mut self) {
        let skill = self.gamestate.difficulty;
        for y in 0..MAPSIZE as u16 {
            for x in 0..MAPSIZE as u16 {
                let code = self.mapspot(x as i32, y as i32, 1);
                if code < 19 {
                    continue;
                }
                // Three skill tiers: (base, tier-1 offset, tier-2 offset).
                let tier = |c: u16, base: u16, step: u16, width: u16| -> Option<u16> {
                    for t in 0..3u16 {
                        let start = base + t * step;
                        if (start..start + width).contains(&c) {
                            let ok = match t {
                                0 => true,
                                1 => skill >= 1,
                                _ => skill >= 2,
                            };
                            return ok.then_some(c - start);
                        }
                    }
                    None
                };
                match code {
                    19..=22 => player::spawn_player(self, x, y, code - 19),
                    23..=85 => self.spawn_static_scan(x as u8, y as u8, (code - 23) as usize),
                    86..=88 => self.spawn_anim_wall(x, y, code - 83),
                    PUSHABLETILE => {
                        if !self.loadedgame {
                            self.gamestate.secrettotal += 1;
                        }
                    }
                    103 => self.spawn_anim_wall(x, y, 6),
                    106 => self.spawn_anim_wall(x, y, 1),
                    107 => self.spawn_anim_wall(x, y, 2),
                    124 => self.spawn_anim_wall(x, y, 6),
                    142 | 143 | 160 | 161 => {
                        let d = if code >= 160 { code - 160 } else { code - 142 };
                        actors::spawn_stand(self, 12, x, y, d);
                    }
                    179 => actors::spawn_special(self, crate::world::class::FAT_MAN, x, y),
                    196 => actors::spawn_special(self, crate::world::class::GUNNER, x, y),
                    197 => actors::spawn_special(self, crate::world::class::MANHOLE, x, y),
                    214 => actors::spawn_special(self, crate::world::class::BOSS, x, y),
                    215 => actors::spawn_special(self, crate::world::class::SWAMP, x, y),
                    232 | 233 | 250 | 251 => {
                        actors::spawn_special(self, crate::world::class::RED_GIANT, x, y)
                    }
                    282..=285 => {
                        actors::spawn_fire_actor(self, x, y);
                    }
                    _ => {
                        if let Some(d) = tier(code, 108, 36, 4) {
                            actors::spawn_stand(self, 0, x, y, d);
                        } else if let Some(d) = tier(code, 112, 36, 4) {
                            actors::spawn_patrol(self, 0, x, y, d, false);
                        } else if let Some(d) = tier(code, 116, 36, 4) {
                            actors::spawn_stand(self, 1, x, y, d);
                        } else if let Some(d) = tier(code, 120, 36, 4) {
                            actors::spawn_patrol(self, 1, x, y, d, false);
                        } else if let Some(i) = tier(code, 126, 36, 16) {
                            actors::spawn_stand(self, 5 + i / 4, x, y, i % 4);
                        } else if let Some(d) = tier(code, 216, 18, 4) {
                            actors::spawn_stand(self, 4, x, y, d);
                        } else if let Some(d) = tier(code, 220, 18, 4) {
                            actors::spawn_patrol(self, 4, x, y, d, false);
                        } else if let Some(d) = tier(code, 224, 18, 4) {
                            allies::spawn_ally(self, x, y, d);
                        } else if let Some(d) = tier(code, 270, 18, 4) {
                            actors::spawn_patrol(self, 3, x, y, d, true);
                        } else if let Some(d) = tier(code, 274, 18, 4) {
                            actors::spawn_patrol(self, 3, x, y, d, false);
                        } else if let Some(d) = tier(code, 278, 18, 4) {
                            actors::spawn_stand(self, 9, x, y, d);
                        }
                    }
                }
            }
        }
    }

    /// `SpawnStatic` from the info plane, with the Random game type's item scatter.
    fn spawn_static_scan(&mut self, x: u8, y: u8, index: usize) {
        let Some(info) = self.data.statinfo.get(index).copied() else { return };
        let bonus = matches!(info.kind, 3..=14 | 20);
        if self.gamestate.difficulty == 3 && bonus {
            // Up to 10 tries at a random empty cell without a static (ghidra 2281:0105).
            for _ in 0..10 {
                let nx = self.random_near(i16::from(x), 24) as u8;
                let ny = self.random_near(i16::from(y), 24) as u8;
                if self.actorat(i32::from(nx), i32::from(ny)).is_empty()
                    && self.static_at(nx, ny).is_none()
                {
                    self.spawn_static(nx, ny, index);
                    return;
                }
            }
            return;
        }
        if self.actorat(i32::from(x), i32::from(y)) == Occ::Mark(1) {
            return;
        }
        self.spawn_static(x, y, index);
    }

    /// Floor/ceiling texture sets for this map (sewers override sets 0–3).
    fn level_floor_sets(&mut self, mapon: i16) {
        let mut sets = self.data.levels.floor_sets;
        if (0..=4).contains(&mapon) {
            sets[..4].copy_from_slice(&SEWER_FLOOR_SETS);
        }
        self.floor_sets = sets;
    }

    /// Song for a level: table for maps 0–9, `US_RndT() % 9` otherwise
    /// (`StartMusic` ghidra 1942:0d2d). Audio chunk = song + 300.
    pub fn level_song(&mut self) -> u16 {
        let m = self.gamestate.mapon;
        if (0..10).contains(&m) {
            self.data.levels.songs[m as usize]
        } else {
            (self.rnd() % 9) as u16
        }
    }
}

impl World {
    /// `ex_completed` bookkeeping after the exit sequence (GAMEFLOW §5.3):
    /// next map and +35 health, capped only by the per-frame 999 limit.
    pub fn advance_level(&mut self) {
        self.gamestate.mapon += 1;
        self.gamestate.health = self.gamestate.health.saturating_add(35);
        self.playstate = ex::STILLPLAYING;
    }
}

/// `starthealth[difficulty]` (DS:0x32EC).
pub fn starthealth(difficulty: i16) -> i16 {
    const T: [i16; 8] = [400, 400, 400, 400, 800, 800, 800, 400];
    T[(difficulty.clamp(0, 7)) as usize]
}
