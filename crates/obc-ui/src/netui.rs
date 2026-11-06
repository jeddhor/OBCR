//! Network game screens and the transport glue (docs/re/NETWORK.md §3, §7;
//! ADR-017): the setup screen (`NetSetup` 358f:0959), the KILLS table
//! (`NetDrawKills` 358f:13c5) and [`NetLink`], which moves packets between
//! the simulation and `obc-net`.
//!
//! OBCR additions on the setup screen: a list of the players heard so far,
//! J to join an address (play over the Internet), and for game type M a
//! direct connection in place of the modem screens (`33bb`).

use std::net::SocketAddr;
use std::time::Instant;

use obc_core::input::{InputState, pad, sc};
use obc_formats::vgagraph::Font;
use obc_net::{Config, Event, Transport};
use obc_render::Framebuffer;
use obc_render::draw2d::{draw_text, text_width};
use obc_sim::net::{self as simnet, NetGame, Packet, kind};
use obc_sim::world::World;

use crate::assets::Assets;
use crate::game::Game;
use crate::screen::{Ctx, Frame, Screen, Transition};
use crate::viewer::tile_background;

/// Characters (DS:0x3928) and teams (DS:0x3922).
const CHARACTERS: [&str; 3] = ["Agent", "Terrorist 1", "Terrorist 2"];
const TEAMS: [&str; 3] = ["Team 1", "Team 2", "Team 3"];
/// Hex keys for the socket number (DS:0x3932).
const HEX_KEYS: [(u8, char); 16] = [
    (0x0B, '0'),
    (0x02, '1'),
    (0x03, '2'),
    (0x04, '3'),
    (0x05, '4'),
    (0x06, '5'),
    (0x07, '6'),
    (0x08, '7'),
    (0x09, '8'),
    (0x0A, '9'),
    (0x1E, 'A'),
    (0x30, 'B'),
    (0x2E, 'C'),
    (0x20, 'D'),
    (0x12, 'E'),
    (0x21, 'F'),
];
/// The J key (join an address).
const KEY_J: u8 = 0x24;
/// Tics "Get Ready!" stays up before the game starts.
const READY_TICS: u32 = 50;

/// The data-release id machines must share.
pub fn release_id(a: &Assets) -> u64 {
    obc_net::release_id(a.release.release.id)
}

/// The transport of a running network game.
pub struct NetLink {
    pub transport: Transport,
    /// Records heard before the game started.
    pending: Vec<Packet>,
}

impl NetLink {
    /// Before a frame: received records into the world (the receive handler).
    pub fn receive(&mut self, w: &mut World) {
        let events = self.transport.poll(Instant::now());
        let mut new_id = None;
        let Some(n) = w.net.as_deref_mut() else { return };
        n.inbox.append(&mut self.pending);
        for e in events {
            match e {
                Event::Record(r) => n.inbox.push(Packet::from_bytes(&r)),
                Event::Left(id) => n.left.push(u16::from(id)),
                Event::IdChanged(id) => {
                    n.my_id = u16::from(id);
                    new_id = Some(i16::from(id));
                }
                Event::Joined { .. } | Event::Mismatch(_) => {}
            }
        }
        // The player's owner tag (`temp2`) follows a changed id at once.
        if let Some(id) = new_id {
            let p = w.player;
            w.obj_mut(p).temp2 = id;
        }
    }

    /// After a frame: what it sent.
    pub fn send(&mut self, w: &mut World) {
        let Some(n) = w.net.as_deref_mut() else { return };
        let out: Vec<_> = n.outbox.drain(..).map(|o| (o.packet.to_bytes(), o.broadcast)).collect();
        if !out.is_empty() {
            self.transport.send_frame(&out, Instant::now());
        }
    }

    /// Keeps the link alive while the game is not running frames.
    pub fn idle(&mut self) {
        for e in self.transport.poll(Instant::now()) {
            if let Event::Record(r) = e {
                self.pending.push(Packet::from_bytes(&r));
            }
        }
        if self.pending.len() > 256 {
            self.pending.drain(..self.pending.len() - 256);
        }
    }
}

fn kind_name(k: i16) -> &'static str {
    match k {
        kind::TEAMS => "NETWORK TEAMS",
        kind::MODEM => "DIRECT CONNECTION",
        _ => "NETWORK",
    }
}

/// `358f:0631`: text with a shadow one pixel up and left.
fn shadowed(fb: &mut Framebuffer, font: &Font, text: &str, x: i32, y: i32, color: u8) {
    draw_text(fb, font, text.as_bytes(), x - 1, y - 1, 0);
    draw_text(fb, font, text.as_bytes(), x, y, color);
}

/// `358f:08d5`: centred text with a shadow one pixel down and right.
fn centred(fb: &mut Framebuffer, font: &Font, text: &str, y: i32, color: u8) {
    let w = text_width(font, text.as_bytes());
    draw_text(fb, font, text.as_bytes(), 161 - w / 2, y + 1, 0);
    draw_text(fb, font, text.as_bytes(), 160 - w / 2, y, color);
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Entry {
    None,
    /// A new socket (port) number, 4 hex digits (`US_LineInput` at 0xED, 0x14).
    Port(String),
    /// An address to join.
    Join(String),
}

/// `NetSetup(type)` (358f:0959).
pub struct NetSetup {
    kind: i16,
    transport: Option<Transport>,
    error: Option<String>,
    choice: i16,
    entry: Entry,
    joined: Vec<SocketAddr>,
    note: Option<String>,
    pending: Vec<Packet>,
    ready: Option<u32>,
}

impl NetSetup {
    pub fn new(ctx: &mut Ctx, k: i16) -> Self {
        let mut s = Self {
            kind: k,
            transport: None,
            error: None,
            choice: ctx.settings.net.choice.clamp(0, 2),
            entry: Entry::None,
            joined: Vec::new(),
            note: None,
            pending: Vec::new(),
            ready: None,
        };
        s.open(ctx);
        s
    }

    fn config(&self, ctx: &Ctx) -> Config {
        let modem = self.kind == kind::MODEM;
        Config {
            port: ctx.settings.net.port,
            game_kind: self.kind as u8,
            release: release_id(ctx.assets),
            join: self.joined.clone(),
            broadcast: !modem,
            max_peers: if modem { 1 } else { obc_net::MAX_PLAYERS - 1 },
            id: None,
            loss_percent: ctx.settings.net.loss_percent,
        }
    }

    fn open(&mut self, ctx: &mut Ctx) {
        self.transport = None;
        let saved = ctx.settings.net.join.clone();
        if !saved.is_empty()
            && self.joined.is_empty()
            && let Ok(a) = obc_net::resolve(&saved, ctx.settings.net.port)
        {
            self.joined.push(a);
        }
        match Transport::open(self.config(ctx), Instant::now()) {
            Ok(t) => {
                self.transport = Some(t);
                self.error = None;
            }
            Err(e) => self.error = Some(format!("{e}")),
        }
    }

    fn players(&self) -> Vec<(u8, SocketAddr)> {
        self.transport.as_ref().map(|t| t.players().collect()).unwrap_or_default()
    }

    fn start(&mut self, ctx: &mut Ctx) -> Transition {
        let Some(transport) = self.transport.take() else { return Transition::Pop };
        ctx.settings.net.choice = self.choice;
        let net = NetGame::new(u16::from(transport.id()), self.choice);
        let link = NetLink { transport, pending: std::mem::take(&mut self.pending) };
        Transition::Reset(Box::new(Game::new_net(ctx, self.kind, net, link)))
    }
}

fn typed_hex(input: &InputState) -> Option<char> {
    HEX_KEYS.iter().find(|(k, _)| input.pressed(*k)).map(|&(_, c)| c)
}

impl Screen for NetSetup {
    fn name(&self) -> &'static str {
        "netsetup"
    }

    fn tick(&mut self, ctx: &mut Ctx) -> Transition {
        let input = ctx.input;
        if let Some(t) = &mut self.transport {
            for e in t.poll(Instant::now()) {
                match e {
                    Event::Record(r) => self.pending.push(Packet::from_bytes(&r)),
                    Event::Mismatch(a) => {
                        self.note = Some(format!("{a}: other version or game type"));
                    }
                    _ => {}
                }
            }
            if self.pending.len() > 64 {
                self.pending.drain(..self.pending.len() - 64);
            }
        }
        if let Some(t) = &mut self.ready {
            *t += 1;
            if *t >= READY_TICS {
                return self.start(ctx);
            }
            return Transition::None;
        }
        if self.error.is_some() {
            // `IPX initialization error`, then back to the menu.
            if input.events.iter().any(|e| e.down && !e.repeat) || input.pad_pressed(pad::SOUTH) {
                return Transition::Pop;
            }
            return Transition::None;
        }
        match &mut self.entry {
            Entry::Port(text) => {
                for c in input.text.chars().filter(char::is_ascii_hexdigit) {
                    if text.len() < 4 {
                        text.push(c.to_ascii_uppercase());
                    }
                }
                if input.pressed(sc::BACKSPACE) {
                    text.pop();
                }
                if input.pressed(sc::ENTER) {
                    // A new socket: close the old one and start over (358f:0d0b).
                    if let Ok(p) = u16::from_str_radix(text, 16)
                        && p >= 0x100
                    {
                        ctx.settings.net.port = p;
                    }
                    self.entry = Entry::None;
                    ctx.requests.text_input = Some(false);
                    self.open(ctx);
                } else if input.pressed(sc::ESCAPE) {
                    self.entry = Entry::None;
                    ctx.requests.text_input = Some(false);
                }
                return Transition::None;
            }
            Entry::Join(text) => {
                for c in input.text.chars().filter(|c| c.is_ascii_graphic()) {
                    if text.len() < 60 {
                        text.push(c);
                    }
                }
                if input.pressed(sc::BACKSPACE) {
                    text.pop();
                }
                if input.pressed(sc::ENTER) {
                    let t = text.clone();
                    self.entry = Entry::None;
                    ctx.requests.text_input = Some(false);
                    match obc_net::resolve(&t, ctx.settings.net.port) {
                        Ok(a) => {
                            if let Some(tr) = &mut self.transport {
                                tr.join(a, Instant::now());
                            }
                            self.joined.retain(|&j| j != a);
                            self.joined.push(a);
                            ctx.settings.net.join = t;
                            self.note = Some(format!("Calling {a}"));
                        }
                        Err(e) => self.note = Some(format!("{t}: {e}")),
                    }
                } else if input.pressed(sc::ESCAPE) {
                    self.entry = Entry::None;
                    ctx.requests.text_input = Some(false);
                }
                return Transition::None;
            }
            Entry::None => {}
        }
        if input.pressed(sc::UP) || input.pad_pressed(pad::DPAD_UP) {
            self.choice = (self.choice + 2) % 3;
        }
        if input.pressed(sc::DOWN) || input.pad_pressed(pad::DPAD_DOWN) {
            self.choice = (self.choice + 1) % 3;
        }
        if input.pressed(sc::ESCAPE) || input.pad_pressed(pad::EAST | pad::BACK) {
            return Transition::Pop;
        }
        if input.pressed(KEY_J) {
            self.entry = Entry::Join(String::new());
            ctx.requests.text_input = Some(true);
            return Transition::None;
        }
        if input.pressed(sc::DELETE) {
            self.joined.clear();
            ctx.settings.net.join.clear();
            self.open(ctx);
            return Transition::None;
        }
        if let Some(c) = typed_hex(input)
            && !input.pressed(KEY_J)
        {
            self.entry = Entry::Port(c.to_string());
            ctx.requests.text_input = Some(true);
            return Transition::None;
        }
        if input.pressed(sc::ENTER)
            || input.pressed(sc::SPACE)
            || input.pressed(sc::CONTROL)
            || input.pad_pressed(pad::SOUTH | pad::START)
        {
            self.ready = Some(0);
        }
        Transition::None
    }

    fn draw(&mut self, a: &Assets, out: &mut Frame) {
        out.palette = a.gamepal;
        out.layer_key = None;
        tile_background(out.layer, a, 1);
        out.frame.clear(0);
        let Some(font) = a.fonts.first() else { return };
        let fb = &mut *out.layer;
        centred(fb, font, kind_name(self.kind), 186, 0x9C);
        if let Some(e) = &self.error {
            centred(fb, font, "Network initialization error", 0x50, 0xBF);
            centred(fb, font, e, 0x64, 0x98);
            return;
        }
        let Some(t) = &self.transport else { return };
        centred(fb, font, &format!("Player ID number {}", t.id()), 2, 0x94);
        let ip = obc_net::local_ip().map_or_else(|| "?".into(), |i| i.to_string());
        centred(fb, font, &format!("UDP address {ip}"), 0x0E, 0x98);
        centred(fb, font, &format!("Communicating on UDP port {:X}", t.port()), 0x1A, 0x9C);
        let title = if self.kind == kind::TEAMS { "CHOOSE A TEAM" } else { "CHOOSE A PLAYER" };
        centred(fb, font, title, 0x28, 0x9F);
        let names = if self.kind == kind::TEAMS { TEAMS } else { CHARACTERS };
        let w = names.iter().map(|n| text_width(font, n.as_bytes())).max().unwrap_or(0);
        let x = 160 - w / 2;
        for (i, n) in names.iter().enumerate() {
            let hl = self.ready.is_none() && i as i16 == self.choice;
            let c = if hl { 0xBF } else { 0x94 + 2 * i as u8 };
            shadowed(fb, font, n, x, 0x3C + 12 * i as i32, c);
        }
        if self.ready.is_some() {
            centred(fb, font, "Get Ready!", 0x78, 0xBF);
            return;
        }
        // OBCR: who is here, and how to reach players elsewhere.
        let players = self.players();
        let line = if players.is_empty() {
            "No other players yet".to_string()
        } else {
            let ids: Vec<String> = players.iter().map(|(id, _)| id.to_string()).collect();
            format!("Players here: {}", ids.join(" "))
        };
        centred(fb, font, &line, 0x6E, 0x98);
        let calling = match self.joined.as_slice() {
            [] if self.kind == kind::MODEM => "Waiting for a call".to_string(),
            [] => "Searching the LAN".to_string(),
            j => {
                let s: Vec<String> = j.iter().map(SocketAddr::to_string).collect();
                format!("Calling {}", s.join(" "))
            }
        };
        centred(fb, font, &calling, 0x7C, 0x94);
        if let Some(n) = &self.note {
            centred(fb, font, n, 0x8A, 0x9A);
        }
        match &self.entry {
            Entry::Port(text) => {
                fb.bar(0xED, 0x14, 0x20, 9 + 4, 0x30);
                draw_text(fb, font, text.as_bytes(), 0xEE, 0x14, 0xBF);
            }
            Entry::Join(text) => {
                centred(fb, font, "Address to call (host or host:port):", 0x98, 0x9F);
                let shown = format!("{text}_");
                let w = text_width(font, shown.as_bytes()).max(120);
                fb.bar(158 - w / 2, 0xA6, w + 4, 15, 0x30);
                draw_text(fb, font, shown.as_bytes(), 160 - w / 2, 0xA7, 0xBF);
            }
            Entry::None => {
                centred(fb, font, "J: CALL   DEL: FORGET   0-F: PORT", 0x9A, 0x94);
                centred(fb, font, "ENTER: START   ESC: BACK", 0xA8, 0x94);
            }
        }
    }
}

/// Row and column positions of the KILLS table (DS:0x3964–0x39FB).
const GRID_X: [i32; 14] = [15, 35, 58, 81, 104, 127, 150, 173, 196, 219, 242, 265, 288, 311];
const GRID_Y: [i32; 14] = [21, 34, 47, 60, 73, 86, 99, 112, 125, 138, 151, 164, 177, 190];
const HEAD_X: [i32; 12] = [42, 65, 88, 111, 134, 157, 180, 203, 226, 246, 271, 292];
const ROW_Y: [i32; 12] = [35, 48, 61, 74, 87, 100, 113, 126, 139, 152, 165, 178];
const CELL_X: [i32; 12] = [37, 60, 83, 106, 129, 152, 175, 198, 221, 244, 267, 290];

/// `NetDrawKills` (358f:13c5): who killed whom, row = killer slot, column =
/// victim slot, `U` = the local player.
pub fn draw_kills(fb: &mut Framebuffer, a: &Assets, n: &NetGame) {
    let Some(font) = a.fonts.first() else { return };
    fb.bar(0, 0, 320, 200, 0);
    for y in (0..200).step_by(2) {
        fb.bar(0, y, 320, 1, 6);
    }
    for i in 0..14 {
        fb.bar(GRID_X[i], 0x15, 1, 0xBE - 0x15 + 1, 0x98);
        fb.bar(0x0F, GRID_Y[i], 0x137 - 0x0F + 1, 1, 0x98);
    }
    let present = |s: usize| n.slot_label(s).is_some();
    for s in 0..simnet::MAXSLOTS {
        let c = if s == 0 { 0x9A } else { (s / 2) as u8 + 0xB8 };
        if s == 0 {
            shadowed(fb, font, "U", HEAD_X[0] + 1, 0x16, c);
            shadowed(fb, font, "U", 0x15, ROW_Y[0], c);
        } else if present(s) {
            shadowed(fb, font, &s.to_string(), HEAD_X[s], 0x16, c);
            let x = if s < 9 { 0x10 } else { 0x12 };
            shadowed(fb, font, &format!("{s:2}"), x, ROW_Y[s], c);
        }
    }
    shadowed(fb, font, "KILLS", 0x94, 5, 0xF3);
    for (i, ch) in ["K", "I", "L", "L", "E", "D"].iter().enumerate() {
        shadowed(fb, font, ch, 3, 0x4A + 13 * i as i32, 0xFA);
    }
    for (row, (&y, line)) in ROW_Y.iter().zip(&n.kills).enumerate() {
        for (col, (&x, &v)) in CELL_X.iter().zip(line).enumerate() {
            if !(present(row) && present(col)) {
                continue;
            }
            let c = if row == 0 || col == 0 { 0x9C } else { (col / 2) as u8 + 0xB8 };
            shadowed(fb, font, &format!("{v:2}"), x, y, c);
        }
    }
}

/// Starts a network game without the setup screen (`obcr --net`, for
/// headless tests and quick LAN games).
pub struct NetBoot {
    pub kind: i16,
    pub choice: i16,
    pub join: Vec<String>,
    /// Developer: respawn here instead of at random (x, y, angle).
    pub place: Option<(i32, i32, i16)>,
}

impl Screen for NetBoot {
    fn name(&self) -> &'static str {
        "netboot"
    }

    fn tick(&mut self, ctx: &mut Ctx) -> Transition {
        let modem = self.kind == kind::MODEM;
        let mut join = Vec::new();
        for j in &self.join {
            match obc_net::resolve(j, ctx.settings.net.port) {
                Ok(a) => join.push(a),
                Err(e) => eprintln!("--net-join {j}: {e}"),
            }
        }
        let cfg = Config {
            port: ctx.settings.net.port,
            game_kind: self.kind as u8,
            release: release_id(ctx.assets),
            join,
            broadcast: !modem,
            max_peers: if modem { 1 } else { obc_net::MAX_PLAYERS - 1 },
            id: None,
            loss_percent: ctx.settings.net.loss_percent,
        };
        match Transport::open(cfg, Instant::now()) {
            Ok(transport) => {
                let net = NetGame::new(u16::from(transport.id()), self.choice);
                let link = NetLink { transport, pending: Vec::new() };
                let mut g = Game::new_net(ctx, self.kind, net, link);
                g.dev_place = self.place;
                Transition::Reset(Box::new(g))
            }
            Err(e) => {
                eprintln!("network: cannot open UDP port {}: {e}", ctx.settings.net.port);
                Transition::Quit
            }
        }
    }

    fn draw(&mut self, _a: &Assets, out: &mut Frame) {
        out.frame.clear(0);
        out.layer.clear(0);
    }
}

/// One line about a network game, for logs and headless tests.
pub fn summary(w: &World) -> Option<String> {
    let n = w.net.as_deref()?;
    let players: Vec<String> = n
        .slots
        .iter()
        .map(|s| format!("{}{}", s.id, if s.puppet.is_some() { "" } else { "?" }))
        .collect();
    let deaths: i16 = (0..simnet::MAXSLOTS).map(|k| n.kills[k][0]).sum();
    Some(format!(
        "net: id {} floor {} players [{}] frags {} deaths {} health {}",
        n.my_id,
        w.gamestate.mapon + 1,
        players.join(" "),
        w.gamestate.score,
        deaths,
        w.gamestate.health
    ))
}

/// Opens the setup screen over the main menu (`obcr --net T --net-setup`).
pub struct SetupBoot {
    pub kind: i16,
}

impl Screen for SetupBoot {
    fn name(&self) -> &'static str {
        "setupboot"
    }

    fn tick(&mut self, ctx: &mut Ctx) -> Transition {
        ctx.settings.game.gametype = self.kind;
        Transition::Reset(Box::new(NetSetup::new(ctx, self.kind)))
    }

    fn draw(&mut self, _a: &Assets, out: &mut Frame) {
        out.frame.clear(0);
        out.layer.clear(0);
    }
}
