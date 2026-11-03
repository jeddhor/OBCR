//! The player: controls, movement, collision, use, weapons, pickups, damage
//! (`WOLFSRC/WL_AGENT.C`, `WL_PLAY.C`; OBC deltas in docs/re/PLAYER.md).
//!
//! Delta check: every function was compared with its `COUNT.EXE` body
//! instruction by instruction (segment `208c`, the player parts of
//! `PlayLoop` `1942:1315`, `CheckKeys` `1942:04ea` partly); the facts and
//! their confidence are in docs/re/PLAYER.md, cited per function. The
//! differences are not yet recorded as `D-###` entries in DELTA_SPEC.md and
//! nothing has been traced in DOSBox-X (M5/M7 acceptance).
//!
//! # Where the hooks go in a frame
//!
//! `PlayLoop` (ghidra 1942:1315) runs, per frame:
//!
//! 1. `PollControls` → [`apply_controls`].
//! 2. Walk bob and knock-back, `madenoise = 0` → [`pre_actors`].
//! 3. `MoveDoors`, `MovePWalls`, `MoveAnimWalls`, healing stations.
//! 4. `DoActor` for every object (the player's think is [`t_player`] /
//!    [`t_attack`]).
//! 5. Palette shifts, `ThreeDRefresh` (the visibility pass sets `FL_VISABLE`,
//!    `viewx`, `transx` and calls [`get_bonus`] for touched items).
//! 6. `TimeCount += tics`, sound updates, the quake counter (DS:0x09D8, one
//!    `US_RndT` when it runs out).
//! 7. Wall-break animation and the health cap → [`post_actors`].
//! 8. The random event roll (DS:0x0AB2, `2390:10f2`, up to two `US_RndT`).
//! 9. `CheckKeys` → [`check_keys`].
//!
//! Steps 3, 5, 6 and 8 belong to other modules; the order matters for
//! `US_RndT` parity.

mod interact;
mod items;
mod movement;
mod weapons;

pub use items::{
    add_screen_splat, clear_screen_splats, get_bonus, give_points, heal_self, start_bonus_flash,
    start_damage_flash, take_damage,
};
pub use weapons::{give_clip, give_weapon, start_wall_hit_at, t_attack};

use crate::allies;
use crate::data::S_PLAYER;
use crate::events::SimEvent;
use crate::world::{AREATILE, NUMAREAS, NUMBUTTONS, ObjId, TicCmd, World, bt, class, ex, fl};

/// Rounds in one magazine (`gamestate.ammo` after a reload).
pub const MAGAZINE: i16 = 30;
/// Clip count cap per weapon (0x13).
pub const MAXCLIPS: i16 = 19;
/// Health, armor, max health and similar caps (0x3E7).
pub const MAXVALUE: i16 = 999;

/// `starthealth[difficulty]` (DS:0x32EC, CONFIRMED file read).
pub const STARTHEALTH: [i16; 8] = [400, 400, 400, 400, 800, 800, 800, 400];

/// Hand-weapon horizontal sway by `walkcount & 15` (DS:0x31DC, CONFIRMED file read).
pub const HAND_SWAY: [i16; 16] = [0, 1, 2, 3, 4, 3, 2, 1, 0, -1, -2, -3, -4, -3, -2, -1];
/// Hand-weapon frame by `walkcount & 15` when not attacking (DS:0x31FC, CONFIRMED file read).
pub const HAND_BOB: [i16; 16] = [7, 7, 6, 5, 4, 5, 6, 7, 7, 7, 6, 5, 4, 5, 6, 7];

/// Scatter range for the Random game type (`DS:0x2FD0 << 3`, DS:0x2FD0 = 3).
const SCATTER_RANGE: i16 = 3 << 3;

/// Sound ids used by the player module (AUDIOMUS numbering, docs/re/PLAYER.md).
pub mod snd {
    /// Weapon fire, `45 + weapon` (`PlayWeaponSound` ghidra 208c:1883).
    pub const WEAPON0: u16 = 45;
    /// Splat on the visor (DS:0x33B4).
    pub const SPLAT: [u16; 2] = [62, 63];
    /// Wall-break sounds (DS:0x0ABA, DS:0x0AB4, DS:0x0ABE).
    pub const WALLHIT_START: [u16; 2] = [65, 66];
    pub const WALLHIT_END: [u16; 3] = [64, 67, 68];
    pub const WALLHIT_CHIP: [u16; 2] = [69, 70];
    /// Booby trap (0x4B).
    pub const BOOBY_TRAP: u16 = 75;
    /// Door-look wall switches (208c:12b5–12db).
    pub const SWITCH_A: u16 = 81;
    pub const SWITCH_B: u16 = 80;
    pub const SWITCH_C: u16 = 78;
    pub const SWITCH_D: u16 = 79;
    /// Generator switch (0x54).
    pub const GENERATOR: u16 = 84;
    /// Weapon and clip pickups (0x55).
    pub const ITEM: u16 = 85;
}

/// One blood splat on the "visor" (DS:0x3312, 8-byte records).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Splat {
    /// Sprite offset 0..7 (sprites 640–647).
    pub shape: i16,
    /// View x, clamped to 64..viewwidth-64.
    pub x: i16,
    /// View y, clamped to ≤ viewheight.
    pub y: i16,
    /// Type `kind % 3`.
    pub kind: i16,
}

/// Maximum number of splats (`AddScreenSplat` refuses at 19).
pub const MAXSPLATS: usize = 19;

/// Module state kept in `World::ply`: player globals that are not in `gamestate`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlayerState {
    /// *walkcount* (DS:0x95BD): hand-bob index, incremented once per frame while moving.
    pub walkcount: i16,
    /// Knock-back trigger (DS:0x099E), set by the booby trap.
    pub knockback_trigger: i16,
    /// Knock-back speed (DS:0x09DE).
    pub knockback_speed: i16,
    /// Wall-break animation running (DS:0x09DA).
    pub wallhit_active: bool,
    /// Wall-break countdown (DS:0x09DC).
    pub wallhit_count: i16,
    /// Wall-break tile (DS:0x9419 x, DS:0x9417 y; misnamed `LeftPosition`/`RightPosition`).
    pub wallhit_x: i16,
    pub wallhit_y: i16,
    /// Muzzle light bits (DS:0xC3D2); bit 2 is set by a shot and cleared on odd frames.
    pub lightbits: u16,
    /// `playerxmove` / `playerymove` (DS:0xCD95 / 0xCD91).
    pub playerxmove: i32,
    pub playerymove: i32,
    /// Hit flag of the last `GunAttack` (DS:0xCD8D).
    pub gunhit: bool,
    /// `exitkind` (DS:0x0922): 0 ladder/stairs (`F3`), 1 elevator (`F4`).
    pub exitkind: i16,
    /// Tab map overlay (DS:0xC3CC): 0 off, then 1, then -1 (stays until the next level).
    pub map_overlay: i16,
    /// `TREMORS` option (DS:0x0D44, default 1).
    pub tremors: bool,
    /// Screen splats (DS:0x3312) and their count (DS:0x33B2).
    pub splats: [Splat; MAXSPLATS],
    pub splatcount: u8,
    /// View geometry used by `GunAttack` and `AddScreenSplat` (`SetViewSize` 1497:1083).
    pub viewwidth: i16,
    pub viewheight: i16,
    /// `centerx` (DS:0x8A20) and `shootdelta` (DS:0x8A1E).
    pub centerx: i16,
    pub shootdelta: i16,
    /// The last tic command, for [`check_keys`] at the end of the frame.
    pub cmd: TicCmd,
    /// Forward / backward key held. The original reads `Keyboard[dirscan[...]]`;
    /// [`TicCmd`] has no raw keys, so these come from the sign of `controly`.
    pub key_forward: bool,
    pub key_backward: bool,
}

impl Default for PlayerState {
    fn default() -> Self {
        let mut s = Self {
            walkcount: 0,
            knockback_trigger: 0,
            knockback_speed: 0,
            wallhit_active: false,
            wallhit_count: 0,
            wallhit_x: 0,
            wallhit_y: 0,
            lightbits: 0,
            playerxmove: 0,
            playerymove: 0,
            gunhit: false,
            exitkind: 0,
            map_overlay: 0,
            tremors: true,
            splats: [Splat::default(); MAXSPLATS],
            splatcount: 0,
            viewwidth: 0,
            viewheight: 0,
            centerx: 0,
            shootdelta: 0,
            cmd: TicCmd::default(),
            key_forward: false,
            key_backward: false,
        };
        // Default `viewsize` 16 (CONFIG.DAT default, docs/re/GAMEFLOW.md §10):
        // 256×128 as in Wolf3D's `NewViewSize`; the presentation layer sets
        // the real size with `set_view_size`.
        // HYPOTHESIS(Q-014): OBC's NewViewSize (1497:119b) uses Wolf3D's rule.
        s.set_view_size(256, 128);
        s
    }
}

impl PlayerState {
    /// The view-geometry part of `SetViewSize(w, h)` (ghidra 1497:1083, CONFIRMED):
    /// full screen above height 160, otherwise width rounded down to 16 and
    /// height to 2; then `centerx = w/2 - 1`, `shootdelta = w/10`.
    #[doc(alias = "SetViewSize")]
    pub fn set_view_size(&mut self, width: i16, height: i16) {
        let (w, h) = if height > 160 { (320, 200) } else { (width & !15, height & !1) };
        self.viewwidth = w;
        self.viewheight = h;
        self.centerx = w / 2 - 1;
        self.shootdelta = w / 10;
    }
}

/// Emits a status-bar redraw request (the original's `Draw*` calls).
fn hud(w: &mut World) {
    w.emit(SimEvent::Hud);
}

/// `GameMessage(fmt, ...)` (ghidra 2f4d:12b5): shown for 125 tics.
fn message(w: &mut World, text: impl Into<String>) {
    w.emit(SimEvent::Message(text.into()));
}

/// Coarse angle to a trig-table index. The original indexes with the raw
/// angle; out-of-range angles would read beyond the table, so wrap instead.
fn trig_index(angle: i16) -> usize {
    angle.rem_euclid(360) as usize
}

/// `SpawnPlayer(x, y, dir)` (ghidra 208c:1324, CONFIRMED; `WOLFSRC/WL_AGENT.C:1100`);
/// `dir` 0..3 = north, east, south, west (plane-1 codes 19–22).
///
/// Deltas: `obclass`/`active` set here; Random game type (difficulty 3)
/// relocates the player when the start cell is occupied. The four neighbour
/// tests (W, E, N, S) pick a cell but `208c:143d` overwrites it with the
/// original start (K candidate), so only the "all four occupied" case moves
/// the player, to a random free cell within ±24 tiles (`RandomNear` 2f4d:1388).
/// Network spawning (`358f:0d7b`) is not ported (M13).
#[doc(alias = "SpawnPlayer")]
pub fn spawn_player(w: &mut World, x: u16, y: u16, dir: u16) {
    // `InitActorList` (1942:0c2f) allocates the player as objlist[0];
    // `World::init_actor_list` does not, so do it here if nothing exists yet.
    if w.objcount == 0 {
        match w.get_new_actor() {
            Some(id) => w.player = id,
            None => return,
        }
    }
    let p = w.player;
    {
        let o = w.obj_mut(p);
        o.obclass = class::PLAYER;
        o.active = 1;
    }
    let mut tx = x as i16;
    let mut ty = y as i16;
    if w.gamestate.difficulty == 3 {
        let (xi, yi) = (i32::from(tx), i32::from(ty));
        let occupied = |w: &World, x: i32, y: i32| !w.actorat(x, y).is_empty();
        if occupied(w, xi, yi)
            && occupied(w, xi - 1, yi)
            && occupied(w, xi + 1, yi)
            && occupied(w, xi, yi - 1)
            && occupied(w, xi, yi + 1)
        {
            // The original loops until it finds a free cell; the table-driven
            // RNG repeats after 64 iterations, so give up after 256.
            let mut found = false;
            for _ in 0..256 {
                let nx = w.random_near(tx, SCATTER_RANGE); // 2f4d:1388, two US_RndT
                let ny = w.random_near(ty, SCATTER_RANGE); // 2f4d:1388, two US_RndT
                if w.actorat(i32::from(nx), i32::from(ny)).is_empty() {
                    tx = nx;
                    ty = ny;
                    found = true;
                    break;
                }
            }
            if !found {
                w.emit(SimEvent::Fault("SpawnPlayer: no free cell for the Random start".into()));
            }
        }
    }
    // The area is the low byte of plane 0 minus AREATILE (byte arithmetic).
    let area = (w.mapspot(i32::from(tx), i32::from(ty), 0) as u8).wrapping_sub(AREATILE as u8);
    {
        let o = w.obj_mut(p);
        o.tilex = tx as u16;
        o.tiley = ty as u16;
        o.areanumber = area;
        o.x = (i32::from(tx) << 16) + 0x8000;
        o.y = (i32::from(ty) << 16) + 0x8000;
        o.state = S_PLAYER;
        let mut angle = 1i16.wrapping_sub(dir as i16).wrapping_mul(90);
        if angle < 0 {
            angle += 360;
        }
        o.angle = angle;
        o.flags = fl::NEVERMARK;
    }
    // TODO(M13): difficulty >= 4 calls the network spawn 358f:0d7b here.
    movement::thrust(w, 0, 0);
    init_areas(w);
}

/// `InitAreas` (ghidra 2281:03c8, CONFIRMED; `WOLFSRC/WL_ACT1.C:316`):
/// only the player's area is reachable.
#[doc(alias = "InitAreas")]
fn init_areas(w: &mut World) {
    let area = w.plr().areanumber as usize;
    if area >= NUMAREAS {
        // QuitNum(605) in the original.
        w.emit(SimEvent::Fault(format!("InitAreas: player area {area} out of range")));
        return;
    }
    w.areabyplayer = [false; NUMAREAS];
    w.areabyplayer[area] = true;
}

/// `PollControls` replacement (ghidra 1942:0400, LIKELY; `WOLFSRC/WL_PLAY.C:455`):
/// `buttonheld = buttonstate`, then the tic command's buttons and movement,
/// clamped to ±100·tics. The input mapper builds `controlx`/`controly` from
/// the devices exactly as `PollKeyboardMove`/`PollMouseMove` do.
#[doc(alias = "PollControls")]
pub fn apply_controls(w: &mut World, cmd: &TicCmd) {
    w.controlx = 0;
    w.controly = 0;
    w.buttonheld = w.buttonstate;
    w.buttonstate = cmd.buttons;
    w.controlx = cmd.controlx;
    w.controly = cmd.controly;
    // The walk-bob test reads the raw arrow keys (1942:1538, 1942:1571); the
    // nearest TicCmd equivalent is the sign of the forward axis.
    w.ply.key_forward = cmd.controly < 0;
    w.ply.key_backward = cmd.controly > 0;
    let max = w.tics.wrapping_mul(100);
    let min = max.wrapping_neg();
    if w.controlx > max {
        w.controlx = max;
    } else if w.controlx < min {
        w.controlx = min;
    }
    if w.controly > max {
        w.controly = max;
    } else if w.controly < min {
        w.controly = min;
    }
    w.ply.cmd = *cmd;
}

/// `T_Player` (ghidra 208c:1bfb, CONFIRMED; `WOLFSRC/WL_AGENT.C:1393`).
///
/// Deltas: no `UpdateFace`, no victory spin; the S/I/Tab handler runs here;
/// `Cmd_Use` runs whenever use is down (it does its own edge test).
#[doc(alias = "T_Player")]
pub fn t_player(w: &mut World, id: ObjId) {
    weapons::check_weapon_change(w);
    option_keys(w);
    if w.buttonstate[bt::USE] {
        interact::cmd_use(w);
    }
    if w.buttonstate[bt::ATTACK] && !w.buttonheld[bt::ATTACK] {
        weapons::cmd_fire(w);
    }
    movement::control_movement(w, id);
    update_plu(w);
}

/// The tail of `T_Player`/`T_Attack`: `plux`/`pluy` (position >> 8, truncated
/// to int) and the tile position.
fn update_plu(w: &mut World) {
    let p = *w.plr();
    w.plux = (p.x >> 8) as i16;
    w.pluy = (p.y >> 8) as i16;
    let o = w.obj_mut(w.player);
    o.tilex = (p.x >> 16) as u16;
    o.tiley = (p.y >> 16) as u16;
}

/// Edge-triggered S, I and Tab keys (ghidra 2f4d:0bd3, CONFIRMED): toggle
/// `gamestate` option bits 0 and 1 and step the map overlay 0 → 1 → −1.
fn option_keys(w: &mut World) {
    if w.buttonstate[bt::TOGGLE_S] && !w.buttonheld[bt::TOGGLE_S] {
        w.buttonheld[bt::TOGGLE_S] = true;
        w.gamestate.options ^= 1;
    }
    if w.buttonstate[bt::TOGGLE_I] && !w.buttonheld[bt::TOGGLE_I] {
        w.buttonheld[bt::TOGGLE_I] = true;
        w.gamestate.options ^= 2;
    }
    if w.buttonstate[bt::MAP] && !w.buttonheld[bt::MAP] {
        w.buttonheld[bt::MAP] = true;
        if w.ply.map_overlay == 1 {
            w.ply.map_overlay = -1;
        } else if w.ply.map_overlay == 0 {
            w.ply.map_overlay = 1;
        }
    }
    // TODO(M13): difficulty >= 4 handles network chat/ally keys here (358f:1ea4, 33bb:*).
}

/// Per-frame PlayLoop extras before the actors run (ghidra 1942:1533–15d7,
/// CONFIRMED by instruction read; docs/re/PLAYER.md §3.4): hand-bob counter,
/// booby-trap knock-back, `madenoise = 0`. Runs once per **frame**, not per tic.
///
/// Delta: the original tests the raw forward/backward arrow keys; this port
/// uses [`PlayerState::key_forward`]/[`PlayerState::key_backward`] from the
/// tic command (they differ only when a mouse or joystick moves forward).
pub fn pre_actors(w: &mut World) {
    let ply = &mut w.ply;
    if w.buttonstate[bt::RUN] || ply.key_forward {
        ply.walkcount = ply.walkcount.wrapping_add(1);
        if ply.knockback_trigger != 0 {
            ply.knockback_speed = ply.knockback_speed.wrapping_sub(3) % 50;
        }
    } else if ply.key_backward {
        ply.walkcount = ply.walkcount.wrapping_add(1);
        if ply.knockback_trigger != 0 {
            ply.knockback_speed = ply.knockback_speed.wrapping_add(3) % 50;
        }
    } else {
        ply.walkcount = 0;
        if ply.knockback_speed > 0 {
            ply.knockback_speed -= 3;
        }
        if ply.knockback_speed < 0 {
            ply.knockback_speed += 3;
        }
        if ply.knockback_trigger != 0 {
            ply.knockback_speed = 42;
            ply.knockback_trigger = 0;
        }
        // Pushes the player backward: controly += 3 * speed (after PollControls' clamp).
        let push = ply.knockback_speed.wrapping_mul(3);
        w.controly = w.controly.wrapping_add(push);
    }
    w.madenoise = false;
}

/// Per-frame PlayLoop extras after the view refresh (ghidra 1942:16cc–18c1,
/// LIKELY from the decompiler, instruction read here): the wall-break
/// animation started by `Cmd_Fire`/[`start_wall_hit_at`] and the health cap.
///
/// Wall-break: while active, the tile number advances when the countdown is
/// even, the countdown drops by one per frame; glass tiles 0x39–0x3C and
/// 0x75–0x78 get plane-1 0x69 (see-through). Sounds at countdown 5 and 1, and
/// on odd `tics` for chip tiles 0x35 and 0x79–0x7C (one `US_RndT` each).
pub fn post_actors(w: &mut World) {
    if w.ply.wallhit_active {
        let (x, y) = (i32::from(w.ply.wallhit_x), i32::from(w.ply.wallhit_y));
        let is_glass = |t: u8| (0x39..=0x3C).contains(&t) || (0x75..=0x78).contains(&t);
        if w.ply.wallhit_count % 2 == 0 && World::in_map(x, y) {
            let i = World::idx(x, y);
            w.tilemap[i] = w.tilemap[i].wrapping_add(1);
        }
        w.ply.wallhit_count = w.ply.wallhit_count.wrapping_sub(1);
        if is_glass(w.tile(x, y)) {
            w.set_mapspot(x, y, 1, 0x69);
        }
        if w.ply.wallhit_count <= 0 {
            w.ply.wallhit_active = false;
            w.ply.wallhit_count = 0;
        }
        let t = w.tile(x, y);
        let (gx, gy) = ((x << 16) + 0x8000, (y << 16) + 0x8000);
        if is_glass(t) {
            if w.ply.wallhit_count == 5 {
                let r = (w.rnd() % 2) as usize; // US_RndT, 1942:17de
                w.play_sound_at(snd::WALLHIT_START[r], gx, gy);
            }
            if w.ply.wallhit_count == 1 {
                let r = (w.rnd() % 3) as usize; // US_RndT, 1942:181f
                w.play_sound_at(snd::WALLHIT_END[r], gx, gy);
            }
        } else if ((0x79..=0x7C).contains(&t) || t == 0x35) && w.tics & 1 != 0 {
            let r = (w.rnd() % 2) as usize; // US_RndT, 1942:1898
            w.play_sound_at(snd::WALLHIT_CHIP[r], gx, gy);
        }
    }
    // Health is capped at 999 every frame (1942:18b3).
    if w.gamestate.health > MAXVALUE {
        w.gamestate.health = MAXVALUE;
    }
}

/// The player-relevant part of `CheckKeys` (ghidra 1942:04ea, LIKELY;
/// `WOLFSRC/WL_PLAY.C:614`) for the special keys carried by the tic command:
/// F8 toggles `TREMORS`; the ally keys go to [`allies::handle_keys`]. Like the
/// original's else-if chain, one key is handled per frame.
///
/// TODO(M7): debug keys and the bogus-mode cheats (`RESTOCK`, `INVINCIBLE`,
/// `HEALTH HELP`, `BEWITCHED`, warp) need tic-command fields first.
#[doc(alias = "CheckKeys")]
pub fn check_keys(w: &mut World) {
    // DELTA-UNCHECKED: only the F8 and ally branches of 1942:04ea were read;
    // the rest of CheckKeys (Esc, F-keys, H/C/W, cheats) is not ported here.
    let cmd = w.ply.cmd;
    if cmd.toggle_tremors {
        // 1942:07bf
        if w.ply.tremors {
            w.ply.tremors = false;
            message(w, "TREMORS OFF");
        } else {
            w.ply.tremors = true;
            message(w, "TREMORS ON");
        }
        return;
    }
    if cmd.ally_mode || cmd.ally_switch || cmd.ally_stay {
        allies::handle_keys(w, &cmd);
    }
}

/// The player resets at the start of `PlayLoop` (ghidra 1942:1315–151e,
/// CONFIRMED): `playstate`, `anglefrac`, buttons, wall-break and knock-back
/// state, BEWITCHED, god mode and the ThermoSensor are cleared, the visor
/// splats are wiped; map 23 greets with `UN BUILDING 2`.
pub fn play_loop_start(w: &mut World) {
    w.playstate = ex::STILLPLAYING;
    w.anglefrac = 0;
    w.buttonstate = [false; NUMBUTTONS];
    w.ply.wallhit_count = 0;
    w.ply.wallhit_active = false;
    w.ply.knockback_trigger = 0;
    w.ply.knockback_speed = 0;
    w.bewitched = false;
    w.godmode = false;
    w.thermosensor = false;
    clear_screen_splats(w);
    if w.gamestate.mapon == 23 {
        message(w, "UN BUILDING 2");
    }
}

/// The `gamestate` part of `NewGame(difficulty)` (ghidra 1497:0549, CONFIRMED;
/// `WOLFSRC/WL_MAIN.C:276`): pistol and Uzi owned, weapon 1, start health,
/// a full magazine, then `ResetInventory`.
#[doc(alias = "NewGame")]
pub fn new_game(w: &mut World, difficulty: i16) {
    w.gamestate = Default::default();
    w.gamestate.difficulty = difficulty;
    w.gamestate.weapons = 1 | 2;
    w.gamestate.chosenweapon = 1;
    w.gamestate.bestweapon = 1;
    w.gamestate.weapon = 1;
    w.gamestate.health = starthealth(difficulty);
    w.gamestate.ammo = MAGAZINE;
    w.gamestate.options = 3;
    w.gamestate.unknown_8722 = 250;
    reset_inventory(w);
    w.gamestate.fuel_cap = 100;
    w.gamestate.maxammo = 200;
    w.gamestate.maxhealth = 100;
}

/// *ResetInventory* (ghidra 2f4d:0f7c, CONFIRMED): armor (200 at difficulty 0),
/// immunity, cheat flag, shot totals, clips {2,0,0,0}, weapon 1. The owned
/// weapons are **not** cleared.
#[doc(alias = "ResetInventory")]
pub fn reset_inventory(w: &mut World) {
    // DS:0x8710 (carried-item count) and the item records are not modelled.
    w.gamestate.armor = 0;
    w.gamestate.immune = 0;
    if w.gamestate.difficulty == 0 {
        w.gamestate.armor = 200;
    }
    w.gamestate.cheatmode = false;
    w.gamestate.shots_total = 0;
    w.gamestate.shots_level = 0;
    w.gamestate.clips = [2, 0, 0, 0];
    w.gamestate.bestweapon = 1;
    w.gamestate.chosenweapon = 1;
    w.gamestate.weapon = 1;
}

/// The per-level player resets of *SetupLevelPlayerState* (ghidra 2f4d:0167,
/// CONFIRMED), skipped when a saved game is loaded: max health and the
/// 0x872C cap back to 100, health reduced to the start health, attack frame
/// and immunity cleared, level shot/hit counts added to the totals, map
/// overlay off.
pub fn setup_level_player_state(w: &mut World) {
    if w.loadedgame {
        return;
    }
    let gs = &mut w.gamestate;
    gs.fuel_cap = 100;
    gs.maxhealth = 100;
    let start = starthealth(gs.difficulty);
    if start < gs.health {
        gs.health = start;
    }
    gs.attackframe = 0;
    gs.immune = 0;
    gs.shots_total = gs.shots_total.wrapping_add(i32::from(gs.shots_level));
    gs.hits_total = gs.hits_total.wrapping_add(i32::from(gs.hits_level));
    gs.hits_level = 0;
    gs.shots_level = 0;
    w.ply.map_overlay = 0;
}

/// `starthealth[difficulty]` with an out-of-range guard.
fn starthealth(difficulty: i16) -> i16 {
    STARTHEALTH.get(difficulty as usize).copied().unwrap_or(400)
}
