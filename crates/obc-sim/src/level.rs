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
pub const SEWER_FLOOR_SETS: [(u8, u8); 4] = [(12, 12), (106, 108), (110, 112), (12, 108)];

impl World {
    /// `SetupGameLevel` for `map` as original floor `mapon`, with that
    /// floor's behaviour ([`LevelProfile::stock`]).
    pub fn setup_game_level(&mut self, map: &Map, mapon: i16) {
        let profile = LevelProfile::stock(mapon, self.data.cd);
        self.setup_game_level_with(map, mapon, profile);
    }

    /// `SetupGameLevel` for `map` as floor `mapon` of the current campaign,
    /// behaving as `profile` says (a mod floor, docs/MODS_AND_OBCEDIT.md §4.1).
    pub fn setup_game_level_with(&mut self, map: &Map, mapon: i16, profile: LevelProfile) {
        self.level = profile;
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
        self.level_floor_sets();
        // Sewers are dark (PlayLoop sets DS:0x33B8 = mapon <= 4).
        self.dark = self.level.dark;
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
                let (what, min_skill) = classify_info(code);
                if skill < min_skill {
                    continue;
                }
                match what {
                    InfoCode::Nothing => {}
                    InfoCode::Player { dir } => player::spawn_player(self, x, y, dir),
                    InfoCode::Static { index } => {
                        self.spawn_static_scan(x as u8, y as u8, usize::from(index))
                    }
                    InfoCode::AnimWall { kind } => self.spawn_anim_wall(x, y, kind),
                    InfoCode::PushWall => {
                        if !self.loadedgame {
                            self.gamestate.secrettotal += 1;
                        }
                    }
                    InfoCode::Stand { which, dir } => actors::spawn_stand(self, which, x, y, dir),
                    InfoCode::Patrol { which, dir, hover } => {
                        actors::spawn_patrol(self, which, x, y, dir, hover)
                    }
                    InfoCode::Special { class } => actors::spawn_special(self, class, x, y),
                    InfoCode::Ally { dir } => allies::spawn_ally(self, x, y, dir),
                    InfoCode::Fire => {
                        actors::spawn_fire_actor(self, x, y);
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
    fn level_floor_sets(&mut self) {
        let mut sets = self.data.levels.floor_sets;
        if self.level.sewer {
            sets[..4].copy_from_slice(&SEWER_FLOOR_SETS);
        }
        self.floor_sets = sets;
    }

    /// Song for a level: table for maps 0–9, `US_RndT() % 9` otherwise
    /// (`StartMusic` ghidra 1942:0d2d). Audio chunk = song + 300.
    pub fn level_song(&mut self) -> u16 {
        match self.level.music {
            LevelMusic::Table(m) => self.data.levels.songs[usize::from(m).min(9)],
            LevelMusic::Song(n) => n,
            LevelMusic::Random => (self.rnd() % 9) as u16,
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

/// What a plane-1 code makes `ScanInfoPlane` spawn (ghidra 1823:01c8, jump
/// table 1823:06c3; docs/re/LEVEL_CODES.md §4). Markers that other code reads
/// (patrol arrows, see-through cells, explosion-proofing) spawn nothing here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InfoCode {
    Nothing,
    /// Player start, compass direction 0 N, 1 E, 2 S, 3 W.
    Player {
        dir: u16,
    },
    /// `statinfo[index]`.
    Static {
        index: u16,
    },
    AnimWall {
        kind: u16,
    },
    PushWall,
    /// `SpawnStand(which, dir)`; `dir` is a `dirtype` / 2 (0 E, 1 N, 2 W, 3 S).
    Stand {
        which: u16,
        dir: u16,
    },
    Patrol {
        which: u16,
        dir: u16,
        hover: bool,
    },
    /// A special spawner by class (bosses, swamp creatures, manholes...).
    Special {
        class: u16,
    },
    Ally {
        dir: u16,
    },
    Fire,
}

/// Classifies a plane-1 code: what it spawns and the lowest game type
/// (`gamestate.difficulty`) that spawns it. Actor codes come in three tiers;
/// the second needs game type 1 or more, the third 2 or more. CONFIRMED.
pub fn classify_info(code: u16) -> (InfoCode, i16) {
    use crate::world::class;
    if code < 19 {
        return (InfoCode::Nothing, 0);
    }
    // Three skill tiers: (base, tier step, width).
    let tier = |base: u16, step: u16, width: u16| -> Option<(u16, i16)> {
        (0..3u16).find_map(|t| {
            let start = base + t * step;
            (start..start + width).contains(&code).then(|| (code - start, t as i16))
        })
    };
    let what = match code {
        19..=22 => InfoCode::Player { dir: code - 19 },
        23..=85 => InfoCode::Static { index: code - 23 },
        86..=88 => InfoCode::AnimWall { kind: code - 83 },
        PUSHABLETILE => InfoCode::PushWall,
        103 | 124 => InfoCode::AnimWall { kind: 6 },
        106 => InfoCode::AnimWall { kind: 1 },
        107 => InfoCode::AnimWall { kind: 2 },
        142 | 143 => InfoCode::Stand { which: 12, dir: code - 142 },
        160 | 161 => InfoCode::Stand { which: 12, dir: code - 160 },
        179 => InfoCode::Special { class: class::FAT_MAN },
        196 => InfoCode::Special { class: class::GUNNER },
        197 => InfoCode::Special { class: class::MANHOLE },
        214 => InfoCode::Special { class: class::BOSS },
        215 => InfoCode::Special { class: class::SWAMP },
        232 | 233 | 250 | 251 => InfoCode::Special { class: class::RED_GIANT },
        282..=285 => InfoCode::Fire,
        _ => {
            let found = [
                (108, 36, 4, 0),
                (112, 36, 4, 1),
                (116, 36, 4, 2),
                (120, 36, 4, 3),
                (126, 36, 16, 4),
                (216, 18, 4, 5),
                (220, 18, 4, 6),
                (224, 18, 4, 7),
                (270, 18, 4, 8),
                (274, 18, 4, 9),
                (278, 18, 4, 10),
            ]
            .into_iter()
            .find_map(|(base, step, width, k)| tier(base, step, width).map(|(i, t)| (k, i, t)));
            let Some((k, i, t)) = found else { return (InfoCode::Nothing, 0) };
            let what = match k {
                0 => InfoCode::Stand { which: 0, dir: i },
                1 => InfoCode::Patrol { which: 0, dir: i, hover: false },
                2 => InfoCode::Stand { which: 1, dir: i },
                3 => InfoCode::Patrol { which: 1, dir: i, hover: false },
                4 => InfoCode::Stand { which: 5 + i / 4, dir: i % 4 },
                5 => InfoCode::Stand { which: 4, dir: i },
                6 => InfoCode::Patrol { which: 4, dir: i, hover: false },
                7 => InfoCode::Ally { dir: i },
                8 => InfoCode::Patrol { which: 3, dir: i, hover: true },
                9 => InfoCode::Patrol { which: 3, dir: i, hover: false },
                _ => InfoCode::Stand { which: 9, dir: i },
            };
            return (what, t);
        }
    };
    (what, 0)
}

/// Where a floor's music comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum LevelMusic {
    /// The song table entry of original floor `n` (0–9).
    Table(u8),
    /// AdLib song `n` (`AUDIOT` chunk 300 + n).
    Song(u16),
    /// `US_RndT() % 9`, as the original does from floor 11 on.
    Random,
}

/// The sequence played after a floor (GAMEFLOW §5.3).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum LevelSequence {
    /// Stairs (`F3`) or elevator (`F4`) by the exit used.
    Auto,
    /// Always this one (`F2` after the sewers, `F5` after floor 23).
    Fixed(String),
    None,
}

/// Everything the original decides from the map slot, as data, so a mod's
/// floor can take any original floor's behaviour (docs/MODS_AND_OBCEDIT.md §4.1).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct LevelProfile {
    /// Starts dark (`DS:0x33B8`, the sewers).
    pub dark: bool,
    /// Sewer floor and ceiling sets and the sewer render tint.
    pub sewer: bool,
    /// Map slot whose roof/floor colour and ramp entries apply ([`crate::data::LevelTables::index`]).
    pub colors: i16,
    pub music: LevelMusic,
    /// The random face apparition can appear (floppy, floors 7 on).
    pub face: bool,
    /// Shown when the floor starts.
    pub message: Option<String>,
    /// The exits refuse to work (the original's floor 40: only Victor ends it).
    pub last: bool,
    pub sequence: LevelSequence,
}

impl LevelProfile {
    /// Original floor `slot`'s behaviour, exactly as the original decides it;
    /// `cd` for the CD release's differences.
    pub fn stock(slot: i16, cd: bool) -> Self {
        Self {
            dark: (0..=4).contains(&slot),
            sewer: (0..=4).contains(&slot),
            colors: slot,
            music: if (0..10).contains(&slot) {
                LevelMusic::Table(slot as u8)
            } else {
                LevelMusic::Random
            },
            face: slot >= 6,
            // PlayLoop (1942:1315); not in BCCD.EXE (CD_RELEASE.md §3.11).
            message: (slot == 23 && !cd).then(|| "UN BUILDING 2".to_owned()),
            last: slot == 39,
            sequence: match slot {
                0..=4 => LevelSequence::Fixed("F2".into()),
                22 => LevelSequence::Fixed("F5".into()),
                _ => LevelSequence::Auto,
            },
        }
    }
}

impl Default for LevelProfile {
    fn default() -> Self {
        Self::stock(0, false)
    }
}
