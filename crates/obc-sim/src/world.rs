//! World state: the original's globals as one struct (DESIGN §7.2.1).

use std::sync::Arc;

use obc_core::rng::Rng;
use obc_core::trig::Tables;

use crate::data::{GameData, StateId};
use crate::events::SimEvent;

/// Objects in `objlist` (`MAXACTORS`, docs/re/ACTORS.md §1).
pub const MAXACTORS: usize = 100;
/// Area numbers 0..22 (`AREATILE` = 0x8C).
pub const NUMAREAS: usize = 22;
pub const AREATILE: u16 = 0x8C;
pub const AMBUSHTILE: u16 = 0x8A;
pub const MAPSIZE: usize = 64;
pub const TILEGLOBAL: i32 = 1 << 16;
pub const MINACTORDIST: i32 = 0x10000;
pub const MINSIGHT: i32 = 0x18000;

/// Index into `objlist`.
pub type ObjId = u8;

/// `actorat[x][y]`: what blocks a tile.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Occ {
    #[default]
    Empty,
    /// A wall or door marker: the original's small integer (walls < 0x80, doors 0x80|n).
    Mark(u16),
    /// An object (`objtype *`).
    Obj(ObjId),
}

impl Occ {
    pub fn is_empty(self) -> bool {
        self == Occ::Empty
    }
}

/// Directions (`dirtype`).
pub mod dir {
    pub const EAST: u16 = 0;
    pub const NORTHEAST: u16 = 1;
    pub const NORTH: u16 = 2;
    pub const NORTHWEST: u16 = 3;
    pub const WEST: u16 = 4;
    pub const SOUTHWEST: u16 = 5;
    pub const SOUTH: u16 = 6;
    pub const SOUTHEAST: u16 = 7;
    pub const NODIR: u16 = 8;

    /// `opposite[]`.
    pub const OPPOSITE: [u16; 9] =
        [WEST, SOUTHWEST, SOUTH, SOUTHEAST, EAST, NORTHEAST, NORTH, NORTHWEST, NODIR];
    /// `dirangle[]` in degrees.
    pub const DIRANGLE: [i32; 9] = [0, 45, 90, 135, 180, 225, 270, 315, 0];

    /// `diagonal[a][b]`.
    pub fn diagonal(a: u16, b: u16) -> u16 {
        match (a, b) {
            (EAST, NORTH) | (NORTH, EAST) => NORTHEAST,
            (NORTH, WEST) | (WEST, NORTH) => NORTHWEST,
            (WEST, SOUTH) | (SOUTH, WEST) => SOUTHWEST,
            (SOUTH, EAST) | (EAST, SOUTH) => SOUTHEAST,
            _ => NODIR,
        }
    }
}

/// `objtype` flags.
pub mod fl {
    pub const SHOOTABLE: u8 = 0x01;
    pub const BONUS: u8 = 0x02;
    pub const NEVERMARK: u8 = 0x04;
    pub const VISABLE: u8 = 0x08;
    pub const ATTACKMODE: u8 = 0x10;
    pub const FIRSTATTACK: u8 = 0x20;
    pub const AMBUSH: u8 = 0x40;
    pub const NONMARK: u8 = 0x80;
    /// OBC `flags2` bit: the actor does not block the player.
    pub const NOBLOCK: u8 = 0x02;
}

/// Object classes (docs/re/ACTORS.md §5).
pub mod class {
    pub const PLAYER: u16 = 1;
    pub const INERT: u16 = 2;
    pub const RAT: u16 = 3;
    pub const SOLDIER: u16 = 4;
    pub const ALLY: u16 = 5;
    pub const SAUCER: u16 = 6;
    pub const GIANT_RAT: u16 = 7;
    pub const STOOL: u16 = 8;
    pub const CROUCH: u16 = 9;
    pub const TOILET: u16 = 10;
    pub const PRONE: u16 = 11;
    pub const BLACK_SUIT: u16 = 12;
    pub const BOSS: u16 = 13;
    pub const GUNNER: u16 = 14;
    pub const RED_VEST: u16 = 15;
    pub const RED_GIANT: u16 = 16;
    pub const FAT_MAN: u16 = 17;
    pub const FLAME: u16 = 18;
    pub const GRENADE: u16 = 19;
    pub const SLIME: u16 = 20;
    pub const FIRE: u16 = 21;
    pub const MANHOLE: u16 = 22;
    pub const SWAMP: u16 = 23;
    pub const EXPLOSION: u16 = 24;
    pub const SPARKLE: u16 = 25;
    pub const FACE: u16 = 26;
}

/// `objtype` (0x42 bytes in OBC; docs/re/ACTORS.md §2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Obj {
    pub active: u16,
    pub ticcount: i16,
    pub obclass: u16,
    pub state: StateId,
    pub flags: u8,
    pub distance: i32,
    pub dir: u16,
    pub x: i32,
    pub y: i32,
    pub tilex: u16,
    pub tiley: u16,
    pub areanumber: u8,
    pub viewx: i16,
    pub viewheight: i16,
    pub transx: i32,
    pub transy: i32,
    pub angle: i16,
    pub hitpoints: i16,
    pub speed: i32,
    pub temp1: i16,
    pub temp2: i16,
    pub temp3: i16,
    pub flags3: u16,
    pub flags2: u8,
    pub target: Option<ObjId>,
    pub next: Option<ObjId>,
    pub prev: Option<ObjId>,
}

/// Door actions (`dr_open` …).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DoorAction {
    #[default]
    Open,
    Closed,
    Opening,
    Closing,
}

/// `doorobj_t` (10 bytes in OBC at DS:0x7BA0).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Door {
    pub tilex: u8,
    pub tiley: u8,
    pub vertical: bool,
    pub lock: u8,
    pub action: DoorAction,
    pub ticcount: i16,
    /// Door kind for rendering (wall pages), from level setup.
    pub kind: u16,
    /// Leading-edge position (`doorposition[]`): 0 closed, 0xFFFF open.
    pub position: u16,
}

/// `statobj_t`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Static {
    pub tilex: u8,
    pub tiley: u8,
    /// Sprite, or -1 when removed.
    pub shape: i16,
    pub flags: u8,
    pub item: i16,
}

/// The moving push wall.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PushWall {
    pub state: u16,
    pub pos: u16,
    pub x: u16,
    pub y: u16,
    pub dir: u16,
    pub tile: u8,
}

/// `gamestate` (DS:0x86FC, docs/re/PLAYER.md §2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct GameState {
    pub difficulty: i16,
    pub mapon: i16,
    pub score: i32,
    pub options: u16,
    pub weapons: u8,
    pub health: i16,
    pub ammo: i16,
    pub bestweapon: i16,
    pub weapon: i16,
    pub chosenweapon: i16,
    pub fuel_cap: i16,
    pub attackframe: i16,
    pub attackcount: i16,
    pub weaponframe: i16,
    pub unknown_8722: i16,
    pub immune: i16,
    pub secretcount: i16,
    pub killcount: i16,
    pub secrettotal: i16,
    pub killtotal: i16,
    pub timecount: i32,
    pub killx: i32,
    pub killy: i32,
    pub cheatmode: bool,
    pub maxammo: i16,
    pub maxhealth: i16,
    pub armor: i16,
    pub shots_level: i16,
    pub shots_total: i32,
    pub hits_level: i16,
    pub hits_total: i32,
    /// `clips[weapon-1]`: 9mm, 7mm, fuel, grenades.
    pub clips: [i16; 4],
}

/// `playstate` values.
pub mod ex {
    pub const STILLPLAYING: u16 = 0;
    pub const COMPLETED: u16 = 1;
    pub const DIED: u16 = 2;
    pub const WARPED: u16 = 3;
    pub const VICTORIOUS: u16 = 6;
    pub const ABORT: u16 = 7;
}

/// Number of player buttons (`NUMBUTTONS`).
pub const NUMBUTTONS: usize = 14;

/// Button indices (docs/re/PLAYER.md §3.1).
pub mod bt {
    pub const ATTACK: usize = 0;
    pub const RUN: usize = 2;
    pub const USE: usize = 3;
    pub const WEAPON1: usize = 4;
    pub const TOGGLE_S: usize = 10;
    pub const TOGGLE_I: usize = 11;
    pub const MAP: usize = 12;
    pub const STRAFE: usize = 13;
}

/// One tic's input (`controlx`, `controly`, `buttonstate`, plus special keys).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TicCmd {
    pub controlx: i16,
    pub controly: i16,
    pub buttons: [bool; NUMBUTTONS],
    /// Ally keys and other CheckKeys actions, edge-triggered.
    pub ally_switch: bool,
    pub ally_stay: bool,
    pub ally_mode: bool,
    pub toggle_tremors: bool,
}

/// The whole simulated world.
pub struct World {
    pub data: Arc<GameData>,
    pub trig: Arc<Tables>,
    pub rng: Rng,
    pub gamestate: GameState,
    pub playstate: u16,
    /// Map planes (`mapsegs`), modified at run time like the original's.
    pub plane0: Vec<u16>,
    pub plane1: Vec<u16>,
    /// `tilemap[x][y]`, index `x*64+y`.
    pub tilemap: Vec<u8>,
    pub actorat: Vec<Occ>,
    pub spotvis: Vec<bool>,
    pub objs: Vec<Obj>,
    pub player: ObjId,
    pub objfreelist: Option<ObjId>,
    pub lastobj: Option<ObjId>,
    pub objcount: u16,
    pub objlistfull: bool,
    pub doors: Vec<Door>,
    pub statics: Vec<Static>,
    pub pwall: PushWall,
    pub areaconnect: [[u8; NUMAREAS]; NUMAREAS],
    pub areabyplayer: [bool; NUMAREAS],
    pub tics: i16,
    pub madenoise: bool,
    pub thrustspeed: i32,
    pub plux: i16,
    pub pluy: i16,
    pub controlx: i16,
    pub controly: i16,
    pub buttonstate: [bool; NUMBUTTONS],
    pub buttonheld: [bool; NUMBUTTONS],
    pub anglefrac: i16,
    pub godmode: bool,
    pub noclip: bool,
    pub bewitched: bool,
    pub killerobj: Option<ObjId>,
    pub lastattacker: Option<ObjId>,
    /// Ally bookkeeping (DS:0x32FC count, DS:0x2A24 mode).
    pub allycount: i16,
    pub allymode: i16,
    /// Map ThermoSensor (DS:0x3304).
    pub thermosensor: bool,
    /// Actor positions for the ThermoSensor overlay (`actorpos`).
    pub actorpos: [u16; MAXACTORS],
    pub events: Vec<SimEvent>,
    /// Level tic counter for diagnostics and traces.
    pub tic: u32,
    pub loadedgame: bool,
    /// Index of the actor DoActor is running (`actorindex`).
    pub actorindex: usize,
    /// State owned by `actors.rs` (enemies, projectiles, fire).
    pub act: crate::actors::ActorsState,
    /// State owned by `player.rs` (controls, weapons, effects).
    pub ply: crate::player::PlayerState,
    /// State owned by `allies.rs`.
    pub ally: crate::allies::AlliesState,
}

impl World {
    /// An empty world (no level loaded) using the given tables.
    pub fn new(data: Arc<GameData>, trig: Arc<Tables>) -> Self {
        let mut w = Self {
            data,
            trig,
            rng: Rng::new(),
            gamestate: GameState::default(),
            playstate: ex::STILLPLAYING,
            plane0: vec![0; MAPSIZE * MAPSIZE],
            plane1: vec![0; MAPSIZE * MAPSIZE],
            tilemap: vec![0; MAPSIZE * MAPSIZE],
            actorat: vec![Occ::Empty; MAPSIZE * MAPSIZE],
            spotvis: vec![false; MAPSIZE * MAPSIZE],
            objs: Vec::new(),
            player: 0,
            objfreelist: None,
            lastobj: None,
            objcount: 0,
            objlistfull: false,
            doors: Vec::new(),
            statics: Vec::new(),
            pwall: PushWall::default(),
            areaconnect: [[0; NUMAREAS]; NUMAREAS],
            areabyplayer: [false; NUMAREAS],
            tics: 1,
            madenoise: false,
            thrustspeed: 0,
            plux: 0,
            pluy: 0,
            controlx: 0,
            controly: 0,
            buttonstate: [false; NUMBUTTONS],
            buttonheld: [false; NUMBUTTONS],
            anglefrac: 0,
            godmode: false,
            noclip: false,
            bewitched: false,
            killerobj: None,
            lastattacker: None,
            allycount: 0,
            allymode: 0,
            thermosensor: false,
            actorpos: [0; MAXACTORS],
            events: Vec::new(),
            tic: 0,
            loadedgame: false,
            actorindex: 0,
            act: Default::default(),
            ply: Default::default(),
            ally: Default::default(),
        };
        w.init_actor_list();
        w
    }

    #[inline]
    pub fn idx(x: i32, y: i32) -> usize {
        (x as usize) * MAPSIZE + y as usize
    }

    pub fn in_map(x: i32, y: i32) -> bool {
        (0..MAPSIZE as i32).contains(&x) && (0..MAPSIZE as i32).contains(&y)
    }

    pub fn actorat(&self, x: i32, y: i32) -> Occ {
        if Self::in_map(x, y) { self.actorat[Self::idx(x, y)] } else { Occ::Mark(1) }
    }

    pub fn set_actorat(&mut self, x: i32, y: i32, v: Occ) {
        if Self::in_map(x, y) {
            self.actorat[Self::idx(x, y)] = v;
        }
    }

    pub fn tile(&self, x: i32, y: i32) -> u8 {
        if Self::in_map(x, y) { self.tilemap[Self::idx(x, y)] } else { 1 }
    }

    /// `MAPSPOT(x,y,plane)`, row-major in the planes.
    pub fn mapspot(&self, x: i32, y: i32, plane: usize) -> u16 {
        if !Self::in_map(x, y) {
            return 0;
        }
        let i = y as usize * MAPSIZE + x as usize;
        if plane == 0 { self.plane0[i] } else { self.plane1[i] }
    }

    pub fn set_mapspot(&mut self, x: i32, y: i32, plane: usize, v: u16) {
        if Self::in_map(x, y) {
            let i = y as usize * MAPSIZE + x as usize;
            if plane == 0 {
                self.plane0[i] = v;
            } else {
                self.plane1[i] = v;
            }
        }
    }

    pub fn rnd(&mut self) -> i16 {
        i16::from(self.rng.rnd_t())
    }

    pub fn obj(&self, id: ObjId) -> &Obj {
        &self.objs[id as usize]
    }

    pub fn obj_mut(&mut self, id: ObjId) -> &mut Obj {
        &mut self.objs[id as usize]
    }

    pub fn plr(&self) -> &Obj {
        &self.objs[self.player as usize]
    }

    pub fn emit(&mut self, e: SimEvent) {
        self.events.push(e);
    }

    /// `InitActorList`.
    pub fn init_actor_list(&mut self) {
        self.objs = vec![Obj::default(); MAXACTORS];
        for i in 0..MAXACTORS {
            self.objs[i].prev = if i + 1 < MAXACTORS { Some(i as ObjId + 1) } else { None };
            self.objs[i].next = None;
        }
        self.objfreelist = Some(0);
        self.lastobj = None;
        self.objcount = 0;
    }

    /// `GetNewActor`: OBC returns `None` instead of quitting when the list is full.
    pub fn get_new_actor(&mut self) -> Option<ObjId> {
        let Some(id) = self.objfreelist else {
            self.objlistfull = true;
            return None;
        };
        let free_next = self.objs[id as usize].prev;
        self.objs[id as usize] = Obj::default();
        self.objfreelist = free_next;
        if let Some(last) = self.lastobj {
            self.objs[last as usize].next = Some(id);
        }
        self.objs[id as usize].prev = self.lastobj;
        self.objs[id as usize].next = None;
        self.lastobj = Some(id);
        self.objcount += 1;
        Some(id)
    }

    /// `RemoveObj`.
    pub fn remove_obj(&mut self, id: ObjId) {
        if id == self.player {
            return; // QuitNum(103) in the original
        }
        let o = self.objs[id as usize];
        self.objs[id as usize].state = 0;
        if Some(id) == self.lastobj {
            self.lastobj = o.prev;
        } else if let Some(n) = o.next {
            self.objs[n as usize].prev = o.prev;
        }
        if let Some(p) = o.prev {
            self.objs[p as usize].next = o.next;
        }
        self.objs[id as usize].prev = self.objfreelist;
        self.objfreelist = Some(id);
        self.objcount = self.objcount.saturating_sub(1);
    }

    /// Objects in list order starting from the player.
    pub fn obj_ids(&self) -> Vec<ObjId> {
        let mut out = Vec::with_capacity(self.objcount as usize);
        let mut cur = Some(self.player);
        while let Some(id) = cur {
            out.push(id);
            cur = self.objs[id as usize].next;
            if out.len() > MAXACTORS {
                break;
            }
        }
        out
    }
}
