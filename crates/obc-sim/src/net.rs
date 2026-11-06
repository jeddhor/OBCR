//! Network game rules: segment `358f` of the original (docs/re/NETWORK.md).
//!
//! Each machine simulates its own copy of the level and is authoritative
//! for its own player. Once per frame [`net_frame`] builds a [`Packet`]
//! describing the local player; other players appear as puppet actors driven
//! by their packets ([`sync_puppet`]). Damage is decided by the victim's
//! machine. The transport (UDP in OBCR, IPX or serial in the original) is
//! outside the simulation: the platform fills [`NetGame::inbox`] before each
//! frame and drains [`NetGame::outbox`] after it, so a frame stays a pure
//! function of its inputs.
//!
//! OBCR differences, all in the transport-facing edge (ADR-017): packets are
//! queued per player instead of overwriting each other within a frame, so no
//! event is lost; puppets are looked up by slot instead of by actor-list
//! position; a player the transport reports as gone is hidden.

use std::collections::VecDeque;

use crate::actors;
use crate::data::StateId;
use crate::player;
use crate::world::{ObjId, World, class, fl};

/// Players in the KILLS table, the local one included (slot 0).
pub const MAXSLOTS: usize = 12;
/// Remote players (`NetRoomForPlayer`, 358f:123e).
pub const MAXREMOTE: usize = MAXSLOTS - 1;
/// First network level (`NETLEVEL 5`).
pub const FIRST_NETLEVEL: i16 = 50;
/// Keep-alive interval in tics (`TimeCount - last > 0x117`, 358f:0eea).
pub const KEEPALIVE_TICS: u32 = 0x118;
/// Packets kept per remote player before old movement-only ones are dropped.
const QUEUE_CAP: usize = 32;

/// Game types (`gamestate.difficulty`).
pub mod kind {
    pub const NETWORK: i16 = 4;
    pub const TEAMS: i16 = 5;
    pub const MODEM: i16 = 6;
}

/// Packet event codes (offset +0).
pub mod ev {
    pub const NONE: u8 = 0;
    pub const MOVING: u8 = 1;
    pub const DIED: u8 = 2;
    pub const USED: u8 = 3;
    pub const FIRED: u8 = 4;
    pub const BURNED: u8 = 5;
    /// "Stopped" nudge after a player joined (forces a send).
    pub const NUDGE: u8 = 0xFF;
}

/// Events a receiver must not lose (everything except movement).
fn significant(event: u8) -> bool {
    matches!(event, ev::DIED | ev::USED | ev::FIRED | ev::BURNED)
}

/// The 24-byte state packet (segment 0x4139 in the original).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Packet {
    pub event: u8,
    pub mapon: u8,
    pub id: u16,
    pub x: i32,
    pub y: i32,
    pub angle: i16,
    pub weapon: i16,
    /// Target id for [`ev::FIRED`] (−1 = none), killer id for deaths.
    pub target: i16,
    /// Character, or team in a team game.
    pub choice: i16,
    /// Frag count (low word of the score).
    pub score: u16,
}

impl Packet {
    pub const SIZE: usize = 24;

    /// The original's layout; bytes 0x14 and 0x15 (receiver flags) are 0.
    pub fn to_bytes(&self) -> [u8; Self::SIZE] {
        let mut b = [0u8; Self::SIZE];
        b[0] = self.event;
        b[1] = self.mapon;
        b[2..4].copy_from_slice(&self.id.to_le_bytes());
        b[4..8].copy_from_slice(&self.x.to_le_bytes());
        b[8..12].copy_from_slice(&self.y.to_le_bytes());
        b[12..14].copy_from_slice(&self.angle.to_le_bytes());
        b[14..16].copy_from_slice(&self.weapon.to_le_bytes());
        b[16..18].copy_from_slice(&self.target.to_le_bytes());
        b[18..20].copy_from_slice(&self.choice.to_le_bytes());
        b[22..24].copy_from_slice(&self.score.to_le_bytes());
        b
    }

    pub fn from_bytes(b: &[u8; Self::SIZE]) -> Self {
        let w = |i: usize| i16::from_le_bytes([b[i], b[i + 1]]);
        let l = |i: usize| i32::from_le_bytes([b[i], b[i + 1], b[i + 2], b[i + 3]]);
        Self {
            event: b[0],
            mapon: b[1],
            id: w(2) as u16,
            x: l(4),
            y: l(8),
            angle: w(12),
            weapon: w(14),
            target: w(16),
            choice: w(18),
            score: w(22) as u16,
        }
    }
}

/// One remote player (the original's slot map DS:0xED7D, record DS:0xEC5B +
/// slot·24 and the per-slot flags at DS:0xEF5D and DS:0xEF75).
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct Slot {
    pub id: u16,
    /// The packet last applied to the puppet.
    pub record: Packet,
    /// The record has been applied (`+0x15`).
    pub seen: bool,
    /// Packets received and not applied yet.
    pub queue: VecDeque<Packet>,
    pub puppet: Option<ObjId>,
    /// Joined; the puppet is spawned by the next [`net_frame`].
    pub fresh: bool,
    /// Alive and shootable since its last spawn or death (DS:0xEF75).
    pub alive: bool,
    /// Walk, shoot or pain animation running (DS:0xEF5D).
    pub anim: bool,
    /// The transport reported this player as gone (OBCR).
    pub gone: bool,
}

/// A packet to send. `broadcast` also announces it on the LAN.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Outgoing {
    pub packet: Packet,
    pub broadcast: bool,
}

/// Network game state (segment `358f` data).
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct NetGame {
    /// Local player id (DS:0x38FC), 1–254.
    pub my_id: u16,
    /// Character or team (DS:0xEF4F), 0–2.
    pub choice: i16,
    /// Remote players; slot number = index + 1.
    pub slots: Vec<Slot>,
    /// `kills[killer slot][victim slot]` (segment 0x4127); slot 0 is local.
    pub kills: [[i16; MAXSLOTS]; MAXSLOTS],
    /// A dropped weapon of this kind is on the floor (DS:0xEF53).
    pub drops: [bool; 5],
    /// Respawn pending (DS:0xEF8D).
    pub respawn: bool,
    /// Reload the level at the next restart (DS:0xEF8F).
    pub reload: bool,
    /// Send the 0xFF nudge once (DS:0xEF91).
    pub nudge: bool,
    /// A remote Use is being replayed (DS:0xEF93).
    pub remote_use: bool,
    /// The packet being built (segment 0x4139).
    pub out: Packet,
    /// The last packet sent (DS:0xEFB3).
    pub last_sent: Option<Packet>,
    /// Tics since the game started (the original reads `TimeCount`).
    pub clock: u32,
    pub last_keepalive: u32,
    /// Filled by the platform before a frame.
    pub inbox: Vec<Packet>,
    /// Ids the transport reports as gone, filled by the platform.
    pub left: Vec<u16>,
    /// Drained by the platform after a frame.
    pub outbox: Vec<Outgoing>,
}

impl NetGame {
    pub fn new(my_id: u16, choice: i16) -> Self {
        Self { my_id, choice: choice.clamp(0, 2), reload: true, ..Self::default() }
    }

    /// Slot number (1-based) of a player id.
    pub fn slot_of_id(&self, id: u16) -> Option<usize> {
        self.slots.iter().position(|s| s.id == id).map(|i| i + 1)
    }

    fn slot_of_obj(&self, ob: ObjId) -> Option<usize> {
        self.slots.iter().position(|s| s.puppet == Some(ob))
    }

    /// Players currently known (remote, not gone).
    pub fn players(&self) -> usize {
        self.slots.iter().filter(|s| !s.gone).count()
    }

    /// The KILLS table label of a slot: the player id, or `None` if empty.
    pub fn slot_label(&self, slot: usize) -> Option<u16> {
        if slot == 0 { Some(self.my_id) } else { self.slots.get(slot - 1).map(|s| s.id) }
    }
}

/// Per-choice tables (DS:0x38FE–0x3920). Index = choice 0–2.
mod choice {
    use crate::data::StateId;
    /// Puppet spawner: 2 = `SpawnAlly`, else the `SpawnStand` type.
    pub const SPAWN: [u16; 3] = [2, 1, 9];
    pub const CLASS: [u16; 3] = [5, 4, 12];
    pub const STAND: [StateId; 3] = [0x2A46, 0x17BE, 0x1C3E];
    pub const WALK: [StateId; 3] = [0x2A46, 0x17CE, 0x1C4E];
    pub const SHOOT: [StateId; 3] = [0x2AC6, 0x184E, 0x1CCE];
    pub const PAIN: [StateId; 3] = [0x2AA6, 0x182E, 0x1CAE];
}

fn ci(choice: i16) -> usize {
    choice.clamp(0, 2) as usize
}

fn state_for(table: &[StateId; 3], choice: i16) -> StateId {
    table[ci(choice)]
}

fn net(w: &World) -> Option<&NetGame> {
    w.net.as_deref()
}

/// `NetMyId` (358f:1995).
pub fn my_id(w: &World) -> u16 {
    net(w).map_or(0, |n| n.my_id)
}

/// `AngleToDir` (358f:0e90): 45° sectors centred on east.
pub fn angle_to_dir(angle: i16) -> u16 {
    match angle {
        a if !(0x17..=0x151).contains(&a) => 0,
        a if a < 0x44 => 1,
        a if a < 0x71 => 2,
        a if a < 0x9E => 3,
        a if a < 0xCB => 4,
        a if a < 0xF8 => 5,
        a if a < 0x125 => 6,
        _ => 7,
    }
}

/// Stores received packets (the receive handler 358f:04d7, run before the
/// frame instead of at interrupt time).
fn absorb_inbox(w: &mut World) {
    let Some(n) = w.net.as_deref_mut() else { return };
    let my = n.my_id;
    for p in std::mem::take(&mut n.inbox) {
        if p.id == my || p.id == 0 {
            continue;
        }
        let i = match n.slots.iter().position(|s| s.id == p.id) {
            Some(i) => {
                n.slots[i].gone = false;
                i
            }
            None => {
                if n.slots.len() >= MAXREMOTE {
                    continue;
                }
                n.slots.push(Slot { id: p.id, fresh: true, ..Slot::default() });
                n.slots.len() - 1
            }
        };
        let q = &mut n.slots[i].queue;
        q.push_back(p);
        while q.len() > QUEUE_CAP {
            // Drop the oldest movement-only packet, or the oldest of all.
            let k = q.iter().position(|p| !significant(p.event)).unwrap_or(0);
            q.remove(k);
        }
    }
    let mut hidden = Vec::new();
    for id in std::mem::take(&mut n.left) {
        if let Some(s) = n.slots.iter_mut().find(|s| s.id == id) {
            s.gone = true;
            s.queue.clear();
            hidden.extend(s.puppet);
        }
    }
    for ob in hidden {
        hide(w, ob);
    }
}

/// Moves a puppet out of play, as for a player on another floor.
fn hide(w: &mut World, ob: ObjId) {
    let o = w.obj_mut(ob);
    o.x = 0;
    o.y = 0;
    o.tilex = 0;
    o.tiley = 0;
    o.ticcount = 0;
    o.flags &= !fl::SHOOTABLE;
}

/// `NetFrame` (358f:0eea): once per frame from the play loop, after the
/// healing stations; also called at once for deaths and Use.
pub fn net_frame(w: &mut World) {
    if w.net.is_none() {
        return;
    }
    absorb_inbox(w);
    if w.net.as_deref().is_some_and(|n| n.respawn) {
        random_spot(w);
        reset_player(w);
    }
    let (cx, cy) = (w.controlx, w.controly);
    let p = *w.plr();
    let g = w.gamestate;
    let Some(n) = w.net.as_deref_mut() else { return };
    if n.out.event == ev::NONE {
        if cx == 0 && cy == 0 {
            if n.nudge {
                n.out.event = ev::NUDGE;
                n.nudge = false;
            }
        } else {
            n.out.event = ev::MOVING;
        }
    }
    n.out.mapon = g.mapon as u8;
    n.out.id = n.my_id;
    n.out.x = p.x;
    n.out.y = p.y;
    n.out.angle = p.angle;
    n.out.choice = n.choice;
    n.out.weapon = g.weapon;
    n.out.score = g.score as u16;
    if n.respawn {
        n.respawn = false;
        n.outbox.push(Outgoing { packet: n.out, broadcast: true });
    } else if n.clock.wrapping_sub(n.last_keepalive) >= KEEPALIVE_TICS {
        n.last_keepalive = n.clock;
        n.outbox.push(Outgoing { packet: n.out, broadcast: true });
    }
    if n.last_sent != Some(n.out) {
        n.last_sent = Some(n.out);
        n.outbox.push(Outgoing { packet: n.out, broadcast: false });
    }
    spawn_new_puppets(w);
    if let Some(n) = w.net.as_deref_mut() {
        n.out.event = ev::NONE;
    }
}

/// The new-player part of `NetFrame`: `PLYR ID %d IN`, a puppet at the
/// player's position, `killcount` = number of remote players.
fn spawn_new_puppets(w: &mut World) {
    loop {
        let Some(n) = w.net.as_deref_mut() else { return };
        let Some(i) = n.slots.iter().position(|s| s.fresh) else { return };
        let s = &mut n.slots[i];
        s.fresh = false;
        let Some(p) = s.queue.front().copied() else { continue };
        let count = n.slots.len() as i16;
        n.nudge = true;
        player::message(w, format!("PLYR ID {} IN", p.id));
        let (tx, ty) = ((p.x >> 16) as u16, (p.y >> 16) as u16);
        let spawner = choice::SPAWN[ci(p.choice)];
        let before = w.lastobj;
        if spawner == 2 {
            crate::allies::spawn_ally(w, tx, ty, 2);
        } else {
            actors::spawn_stand(w, spawner, tx, ty, 2);
        }
        let new = w.lastobj.filter(|&o| Some(o) != before);
        if let Some(ob) = new {
            let o = w.obj_mut(ob);
            o.active = 1;
            o.angle = p.angle;
            o.dir = angle_to_dir(p.angle);
            o.hitpoints = 100;
            o.ticcount = 1;
        }
        if let Some(n) = w.net.as_deref_mut() {
            n.slots[i].puppet = new;
        }
        w.gamestate.killcount = count;
        player::hud(w);
    }
}

/// `NetSyncPuppet(i, ob)` (358f:19fd), from `DoActor` after `actorat` is
/// cleared, for actors of class 4, 5 or 12.
pub fn sync_puppet(w: &mut World, ob: ObjId) {
    let state = w.obj(ob).state;
    let Some(n) = w.net.as_deref_mut() else { return };
    let Some(si) = n.slot_of_obj(ob) else { return };
    let my = n.my_id;
    let mychoice = n.choice;
    let s = &mut n.slots[si];
    if s.gone {
        return;
    }
    let Some(p) = take_packet(&mut s.queue) else {
        // No new data: back to standing once a walk ends.
        let choice = s.record.choice;
        if s.anim && s.record.event == ev::NONE && state != state_for(&choice::PAIN, choice) {
            s.anim = false;
            w.obj_mut(ob).state = state_for(&choice::STAND, choice);
        }
        return;
    };
    s.record = p;
    s.seen = true;
    let slot = si + 1;
    let alive = s.alive;
    let anim = s.anim;

    {
        let o = w.obj_mut(ob);
        o.obclass = choice::CLASS[ci(p.choice)];
        o.ticcount = 1;
        o.x = p.x;
        o.y = p.y;
        o.angle = p.angle;
        o.dir = angle_to_dir(p.angle);
        o.tilex = (p.x >> 16) as u16;
        o.tiley = (p.y >> 16) as u16;
        o.temp1 = p.choice;
        o.temp2 = i16::from(p.id as u8);
        o.temp3 = p.weapon;
    }
    // Team score: team-mates share the highest score.
    let g = &mut w.gamestate;
    if g.difficulty == kind::TEAMS
        && p.choice == mychoice
        && (g.score < 0 || (g.score < 0x10000 && (g.score as u16) < p.score))
    {
        g.score = i32::from(p.score);
    }

    if p.mapon != w.gamestate.mapon as u8 {
        hide(w, ob);
        player::message(w, format!("Activity on {}", u16::from(p.mapon) + 1));
        return;
    }
    let set_slot = |w: &mut World, f: &dyn Fn(&mut Slot)| {
        if let Some(n) = w.net.as_deref_mut() {
            f(&mut n.slots[si]);
        }
    };
    match p.event {
        ev::DIED | ev::BURNED => {
            if p.event == ev::BURNED {
                actors::spawn_burning_flames(w, ob);
            }
            set_slot(w, &|s| s.alive = false);
            actors::kill_actor(w, ob);
            let n = w.net.as_deref_mut().expect("net game");
            let killer = if p.target as u16 == my {
                Some(0)
            } else if p.target >= 0 {
                n.slot_of_id(p.target as u16)
            } else {
                None
            };
            if let Some(k) = killer
                && k < MAXSLOTS
                && slot < MAXSLOTS
            {
                n.kills[k][slot] = n.kills[k][slot].wrapping_add(1);
            }
            if p.target as u16 == my || id_on_my_team(w, p.target) {
                w.gamestate.score = w.gamestate.score.wrapping_add(1);
                player::hud(w);
            }
        }
        _ if !alive => {
            let o = w.obj_mut(ob);
            o.flags |= fl::SHOOTABLE;
            o.distance = crate::world::TILEGLOBAL;
            o.state = state_for(&choice::STAND, p.choice);
            set_slot(w, &|s| s.alive = true);
        }
        ev::FIRED
            if choice::CLASS[ci(p.choice)] == class::ALLY
                || p.target as u16 == my
                || p.weapon > 2 =>
        {
            remote_fire(w, ob);
            set_slot(w, &|s| s.anim = true);
        }
        ev::USED => remote_use(w, ob),
        ev::MOVING if !anim => {
            w.obj_mut(ob).state = state_for(&choice::WALK, p.choice);
            set_slot(w, &|s| s.anim = true);
        }
        ev::NONE if anim && w.obj(ob).state != state_for(&choice::PAIN, p.choice) => {
            w.obj_mut(ob).state = state_for(&choice::STAND, p.choice);
            set_slot(w, &|s| s.anim = false);
        }
        _ => {}
    }
}

/// The next packet to apply: events in order, movement-only packets skipped
/// while a newer one is waiting.
fn take_packet(q: &mut VecDeque<Packet>) -> Option<Packet> {
    while q.len() > 1 && !significant(q[0].event) {
        q.pop_front();
    }
    q.pop_front()
}

/// `NetIdOnMyTeam(id)` (358f:19b7).
fn id_on_my_team(w: &World, id: i16) -> bool {
    let Some(n) = net(w) else { return false };
    if w.gamestate.difficulty != kind::TEAMS || id < 0 {
        return false;
    }
    n.slots.iter().any(|s| s.id == id as u16 && s.record.choice == n.choice)
}

/// `NetTargetsMe(ob)` (358f:1259): the puppet's player last targeted the
/// local player.
pub fn targets_me(w: &World, ob: ObjId) -> bool {
    let Some(n) = net(w) else { return false };
    let id = w.obj(ob).temp2 as u16;
    n.slots.iter().find(|s| s.id == id).is_some_and(|s| s.record.target as u16 == n.my_id)
}

/// `NetSameTeam(ob)` (358f:199d).
pub fn same_team(w: &World, ob: ObjId) -> bool {
    net(w).is_some_and(|n| w.obj(ob).temp1 == n.choice)
}

/// `NetRemoteFire(ob)` (358f:1283): replays a remote shot by weapon.
fn remote_fire(w: &mut World, ob: ObjId) {
    let o = *w.obj(ob);
    let player = w.player;
    match o.temp3 {
        0..=2 if targets_me(w, ob) || o.obclass == class::ALLY => {
            w.obj_mut(ob).state = state_for(&choice::SHOOT, o.temp1);
        }
        3 => {
            let before = w.lastobj;
            actors::spawn_flame_from(w, ob, None);
            if let Some(f) = w.lastobj.filter(|&f| Some(f) != before) {
                let f = w.obj_mut(f);
                f.temp2 = o.temp2;
                f.target = Some(player);
            }
        }
        4 => {
            let before = w.lastobj;
            as_player(w, ob, |w| actors::spawn_grenade(w, None));
            if let Some(g) = w.lastobj.filter(|&g| Some(g) != before) {
                let g = w.obj_mut(g);
                g.temp2 = o.temp2;
                g.target = Some(player);
            }
        }
        _ => {}
    }
}

/// Runs `f` with the puppet standing in for the player, as the original's
/// 0x42-byte copies do; the actor list links of the player are kept.
fn as_player(w: &mut World, ob: ObjId, f: impl FnOnce(&mut World)) {
    let pl = w.player;
    let saved = *w.obj(pl);
    let mut stand_in = *w.obj(ob);
    stand_in.next = saved.next;
    stand_in.prev = saved.prev;
    *w.obj_mut(pl) = stand_in;
    if let Some(n) = w.net.as_deref_mut() {
        n.remote_use = true;
    }
    f(w);
    if let Some(n) = w.net.as_deref_mut() {
        n.remote_use = false;
    }
    let (next, prev) = (w.obj(pl).next, w.obj(pl).prev);
    *w.obj_mut(pl) = saved;
    let p = w.obj_mut(pl);
    p.next = next;
    p.prev = prev;
}

/// The Use event: `Cmd_Use` with the puppet standing in for the player.
fn remote_use(w: &mut World, ob: ObjId) {
    as_player(w, ob, player::remote_cmd_use);
}

/// `NetUseInProgress` (358f:198d).
pub fn use_in_progress(w: &World) -> bool {
    net(w).is_some_and(|n| n.remote_use)
}

/// `NetReportUse` (358f:1979): event 3, sent at once.
pub fn report_use(w: &mut World) {
    if let Some(n) = w.net.as_deref_mut() {
        n.out.event = ev::USED;
    }
    net_frame(w);
}

/// `NetReportFire(flag, target)` (358f:13a2).
pub fn report_fire(w: &mut World, target: i16) {
    if let Some(n) = w.net.as_deref_mut() {
        n.out.event = ev::FIRED;
        n.out.target = target;
    }
}

/// `NetHitPuppet(ob)` (358f:135a): a local hit only shows pain.
pub fn hit_puppet(w: &mut World, ob: ObjId) {
    if ob == w.player {
        return;
    }
    let o = *w.obj(ob);
    let Some(n) = w.net.as_deref_mut() else { return };
    if w.gamestate.difficulty == kind::TEAMS && o.temp1 == n.choice {
        return;
    }
    let Some(si) = n.slots.iter().position(|s| s.id == o.temp2 as u16) else { return };
    n.slots[si].anim = true;
    let p = w.obj_mut(ob);
    p.state = state_for(&choice::PAIN, o.temp1);
    p.ticcount = 1;
}

/// `NetPlayerDied(attacker)` (358f:182c): counts the death and announces it.
pub fn player_died(w: &mut World, attacker: Option<ObjId>) {
    let (aid, aclass) = match attacker {
        Some(a) => (w.obj(a).temp2, w.obj(a).obclass),
        None => (-1, 0),
    };
    let Some(n) = w.net.as_deref_mut() else { return };
    if aid as u16 == n.my_id {
        n.kills[0][0] = n.kills[0][0].wrapping_add(1);
    } else if aid >= 0
        && let Some(k) = n.slot_of_id(aid as u16)
        && k < MAXSLOTS
    {
        n.kills[k][0] = n.kills[k][0].wrapping_add(1);
    }
    n.out.event = if aclass == class::FLAME { ev::BURNED } else { ev::DIED };
    n.out.target = aid;
    net_frame(w);
}

/// `NetDropWeapon(ob, x, y)` (358f:18e7): a dead puppet drops its weapon,
/// one of each kind at a time.
pub fn drop_weapon(w: &mut World, ob: ObjId, x: u8, y: u8) {
    let weapon = w.obj(ob).temp3;
    let Some(n) = w.net.as_deref_mut() else { return };
    let Some(flag) = usize::try_from(weapon).ok().and_then(|i| n.drops.get_mut(i)) else {
        return;
    };
    if *flag {
        return;
    }
    *flag = true;
    let item = match weapon {
        1 => 11,
        2 => 5,
        3 => 12,
        4 => 13,
        _ => return,
    };
    w.place_item_type(item, x, y);
}

/// `NetClearDrop(weapon)` (358f:1969).
pub fn clear_drop(w: &mut World, weapon: usize) {
    if let Some(f) = w.net.as_deref_mut().and_then(|n| n.drops.get_mut(weapon)) {
        *f = false;
    }
}

/// `NetRequestRespawn` (358f:0d7b), from `SpawnPlayer` and after a death.
pub fn request_respawn(w: &mut World) {
    if let Some(n) = w.net.as_deref_mut() {
        n.respawn = true;
        n.reload = false;
    }
}

/// `NetReloadLevel` (358f:1251).
pub fn reload_level(w: &World) -> bool {
    net(w).is_none_or(|n| n.reload)
}

/// `NetRandomSpot` (358f:0d8c): a random empty cell with a free neighbour
/// and no static object.
fn random_spot(w: &mut World) {
    for _ in 0..100_000 {
        let x = w.random_near(0, 64);
        let y = w.random_near(0, 64);
        let (xi, yi) = (i32::from(x), i32::from(y));
        if !w.actorat(xi, yi).is_empty() {
            continue;
        }
        let busy = |dx: i32, dy: i32| !w.actorat(xi + dx, yi + dy).is_empty();
        if busy(1, 0) && busy(0, 1) && busy(-1, 0) && busy(0, -1) {
            continue;
        }
        if w.statics.iter().any(|s| i32::from(s.tilex) == xi && i32::from(s.tiley) == yi) {
            continue;
        }
        let area = (w.mapspot(xi, yi, 0) as u8).wrapping_add(0x74);
        let p = w.player;
        let o = w.obj_mut(p);
        o.tilex = x as u16;
        o.tiley = y as u16;
        o.x = (xi << 16) | 0x8000;
        o.y = (yi << 16) | 0x8000;
        o.areanumber = area;
        return;
    }
}

/// `NetResetPlayer` (358f:1796): the respawn loadout.
pub fn reset_player(w: &mut World) {
    let g = &mut w.gamestate;
    g.health = crate::level::starthealth(g.difficulty);
    g.ammo = player::MAGAZINE;
    g.options = 3;
    g.unknown_8722 = 250;
    player::reset_inventory(w);
    let Some(n) = w.net.as_deref_mut() else { return };
    let choice = n.choice;
    n.drops = [false; 5];
    let my = n.my_id;
    let g = &mut w.gamestate;
    if choice::CLASS[ci(choice)] == class::BLACK_SUIT {
        g.weapons = 5;
        g.chosenweapon = 2;
        g.bestweapon = 2;
        g.weapon = 2;
    } else {
        g.weapons = 3;
        g.chosenweapon = 1;
        g.bestweapon = 1;
        g.weapon = 1;
    }
    g.fuel_cap = 100;
    g.maxammo = 200;
    g.maxhealth = 100;
    let weapon = g.weapon;
    let p = w.player;
    let o = w.obj_mut(p);
    o.temp1 = choice;
    o.temp2 = my as i16;
    o.temp3 = weapon;
}

/// `NetLevelDone` (358f:1720), before the KILLS table: forget every player
/// (they re-join on the next floor) and respawn on a reloaded level.
pub fn level_done(w: &mut World) {
    if let Some(n) = w.net.as_deref_mut() {
        n.respawn = true;
        n.reload = true;
        n.slots.clear();
        n.last_sent = None;
    }
}

/// After a death in a network game (*GameLoop* 1823:1060): respawn on the
/// same level without reloading it.
pub fn respawn_after_death(w: &mut World) {
    request_respawn(w);
    reset_player(w);
    w.playstate = crate::world::ex::STILLPLAYING;
}

/// Advances the network clock; called once per frame by [`World::frame`].
pub(crate) fn tick(w: &mut World) {
    let t = w.tics as u32;
    if let Some(n) = w.net.as_deref_mut() {
        n.clock = n.clock.wrapping_add(t);
    }
}

/// True if `ob` blocks a projectile: a shootable actor of another player,
/// or the local player's cell for someone else's projectile
/// (`ProjectileTryMove`, 2390:0006).
pub fn player_blocks_projectile(w: &World, proj: ObjId, x: i32, y: i32) -> bool {
    let p = w.plr();
    i32::from(p.tilex) == x && i32::from(p.tiley) == y && w.obj(proj).temp2 as u16 != my_id(w)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packet_layout_matches_the_original() {
        let p = Packet {
            event: ev::FIRED,
            mapon: 50,
            id: 0x2A,
            x: 0x0012_8000,
            y: -2,
            angle: 270,
            weapon: 3,
            target: -1,
            choice: 2,
            score: 513,
        };
        let b = p.to_bytes();
        assert_eq!(b[0], 4);
        assert_eq!(b[1], 50);
        assert_eq!(&b[2..4], &[0x2A, 0]);
        assert_eq!(&b[4..8], &[0x00, 0x80, 0x12, 0x00]);
        assert_eq!(&b[16..18], &[0xFF, 0xFF]);
        assert_eq!((b[20], b[21]), (0, 0), "receiver flags are zero");
        assert_eq!(&b[22..24], &[1, 2]);
        assert_eq!(Packet::from_bytes(&b), p);
    }

    #[test]
    fn angle_to_dir_sectors() {
        let cases = [(0, 0), (22, 0), (23, 1), (67, 1), (68, 2), (112, 2), (113, 3)];
        for (a, d) in cases {
            assert_eq!(angle_to_dir(a), d, "angle {a}");
        }
        assert_eq!(angle_to_dir(338), 0);
        assert_eq!(angle_to_dir(337), 7);
    }

    #[test]
    fn take_packet_keeps_events_and_skips_stale_movement() {
        let m = |x| Packet { event: ev::MOVING, x, ..Packet::default() };
        let d = Packet { event: ev::DIED, ..Packet::default() };
        let mut q: VecDeque<Packet> = [m(1), m(2), d, m(3), m(4)].into_iter().collect();
        assert_eq!(take_packet(&mut q), Some(d));
        assert_eq!(take_packet(&mut q), Some(m(4)));
        assert_eq!(take_packet(&mut q), None);
    }
}
