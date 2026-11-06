//! UDP transport for network games (ADR-017; the original's IPX layer is
//! described in docs/re/NETCOMMS.md).
//!
//! The simulation produces 24-byte state records ([`Record`]); this crate
//! carries them between machines. It keeps the original's model, in which
//! every machine sends its own state to every player it knows and announces
//! itself by broadcast. It adds what IPX on a LAN did not need:
//!
//! - a versioned header with the data-release id and game type, so
//!   mismatched machines ignore each other;
//! - joining by address for play over the Internet, plus peer-list gossip
//!   so that every player learns every other;
//! - sequence numbers, the last three records in every datagram and two
//!   delayed resends of the newest one, so single losses are recovered and
//!   duplicates dropped;
//! - player ids that avoid collisions (the original used the last byte of
//!   the IPX node address);
//! - keep-alive beacons, a time-out and a goodbye message.
//!
//! Time is passed in by the caller, so tests can drive it.

use std::collections::VecDeque;
use std::collections::hash_map::RandomState;
use std::hash::{BuildHasher, Hasher};
use std::io;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, ToSocketAddrs, UdpSocket};
use std::time::{Duration, Instant};

/// Size of a state record (the original's packet).
pub const RECORD: usize = 24;
pub type Record = [u8; RECORD];
/// The original's default IPX socket number, used as the UDP port.
pub const DEFAULT_PORT: u16 = 0x4003;
/// Bumped whenever the datagram layout or the game rules change.
pub const PROTOCOL: u8 = 1;
/// A player not heard from for this long has left.
pub const TIMEOUT: Duration = Duration::from_secs(15);
/// Most players in a game, the local one included (the KILLS table size).
pub const MAX_PLAYERS: usize = 12;

const MAGIC: [u8; 4] = *b"OBCR";
const HEADER: usize = 24;
const REDUNDANCY: usize = 3;
const BEACON: Duration = Duration::from_secs(2);
const GOSSIP: Duration = Duration::from_secs(3);
const TAIL: [Duration; 2] = [Duration::from_millis(100), Duration::from_millis(300)];
const MAX_GOSSIP: usize = 16;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    State = 1,
    Hello = 2,
    Peers = 3,
    Bye = 4,
}

impl Kind {
    fn from_u8(v: u8) -> Option<Self> {
        Some(match v {
            1 => Self::State,
            2 => Self::Hello,
            3 => Self::Peers,
            4 => Self::Bye,
            _ => return None,
        })
    }
}

/// Transport settings.
#[derive(Debug, Clone)]
pub struct Config {
    /// UDP port to bind (0 = any, for tests).
    pub port: u16,
    /// Game type (4 network, 5 teams, 6 direct); machines must agree.
    pub game_kind: u8,
    /// Data-release id ([`release_id`]); machines must agree.
    pub release: u64,
    /// Addresses to contact (play over the Internet).
    pub join: Vec<SocketAddr>,
    /// Announce on the LAN by broadcast.
    pub broadcast: bool,
    /// Most remote players accepted.
    pub max_peers: usize,
    /// Force the player id (tests); otherwise random.
    pub id: Option<u8>,
    /// Simulated loss of outgoing datagrams in percent (tests, `--net-loss`).
    pub loss_percent: u8,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            port: DEFAULT_PORT,
            game_kind: 4,
            release: 0,
            join: Vec::new(),
            broadcast: true,
            max_peers: MAX_PLAYERS - 1,
            id: None,
            loss_percent: 0,
        }
    }
}

/// What [`Transport::poll`] reports.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    /// A state record from another player, in order, each once.
    Record(Record),
    /// A player was heard for the first time.
    Joined { id: u8, addr: SocketAddr },
    /// A player said goodbye or timed out.
    Left(u8),
    /// The local id collided with another player's and was changed.
    IdChanged(u8),
    /// A machine with another data release, game type or protocol.
    Mismatch(SocketAddr),
}

/// A known remote machine.
#[derive(Debug, Clone)]
pub struct Peer {
    pub addr: SocketAddr,
    pub id: Option<u8>,
    nonce: u64,
    added: Instant,
    last_heard: Option<Instant>,
    last_seq: Option<u32>,
    /// Given by the user; kept after a time-out.
    pub joined: bool,
}

impl Peer {
    /// Heard from at least once.
    pub fn heard(&self) -> bool {
        self.last_heard.is_some()
    }
}

/// Data-release id for the handshake: FNV-1a of the release name.
pub fn release_id(name: &str) -> u64 {
    let mut h: u64 = 0xCBF2_9CE4_8422_2325;
    for b in name.bytes() {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0100_0000_01B3);
    }
    h
}

/// Parses `host`, `host:port`, `a.b.c.d`, `[v6]:port`; the port defaults to
/// `default_port`.
pub fn resolve(text: &str, default_port: u16) -> io::Result<SocketAddr> {
    let t = text.trim();
    if let Ok(a) = t.parse::<SocketAddr>() {
        return Ok(a);
    }
    if let Ok(ip) = t.parse::<IpAddr>() {
        return Ok(SocketAddr::new(ip, default_port));
    }
    let with_port = if t.contains(':') { t.to_string() } else { format!("{t}:{default_port}") };
    with_port
        .to_socket_addrs()?
        .find(SocketAddr::is_ipv4)
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "no IPv4 address"))
}

/// The machine's LAN address as the OS would route outward (no packet is
/// sent), for the setup screen.
pub fn local_ip() -> Option<IpAddr> {
    let s = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0)).ok()?;
    s.connect((Ipv4Addr::new(192, 0, 2, 1), 9)).ok()?;
    s.local_addr().ok().map(|a| a.ip())
}

fn random_u64() -> u64 {
    let mut h = RandomState::new().build_hasher();
    h.write_u64(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos() as u64),
    );
    h.finish()
}

/// Newer-than for wrapping sequence numbers.
fn seq_newer(a: u32, b: u32) -> bool {
    a != b && a.wrapping_sub(b) < 0x8000_0000
}

#[derive(Debug)]
struct Header {
    kind: Kind,
    game_kind: u8,
    id: u8,
    release: u64,
    nonce: u64,
}

/// The UDP endpoint of one player.
#[derive(Debug)]
pub struct Transport {
    sock: UdpSocket,
    cfg: Config,
    nonce: u64,
    id: u8,
    seq: u32,
    history: VecDeque<(u32, Record)>,
    peers: Vec<Peer>,
    last_send: Option<Instant>,
    tail: usize,
    last_beacon: Option<Instant>,
    last_gossip: Option<Instant>,
    mismatched: Vec<SocketAddr>,
    /// Addresses that turned out to be this machine.
    self_addrs: Vec<SocketAddr>,
    loss: u32,
    sent_datagrams: u64,
}

impl Transport {
    /// Binds the port and starts announcing. `now` starts the clocks.
    pub fn open(cfg: Config, now: Instant) -> io::Result<Self> {
        let sock = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, cfg.port))?;
        sock.set_nonblocking(true)?;
        sock.set_broadcast(true)?;
        let nonce = random_u64();
        let id = cfg.id.unwrap_or((nonce % 254) as u8 + 1);
        let peers = cfg
            .join
            .iter()
            .map(|&addr| Peer {
                addr,
                id: None,
                nonce: 0,
                added: now,
                last_heard: None,
                last_seq: None,
                joined: true,
            })
            .collect();
        Ok(Self {
            sock,
            nonce,
            id,
            seq: 0,
            history: VecDeque::new(),
            peers,
            last_send: None,
            tail: TAIL.len(),
            last_beacon: None,
            last_gossip: None,
            mismatched: Vec::new(),
            self_addrs: Vec::new(),
            loss: (nonce as u32) | 1,
            sent_datagrams: 0,
            cfg,
        })
    }

    /// The local player id, 1–254.
    pub fn id(&self) -> u8 {
        self.id
    }

    /// The bound port.
    pub fn port(&self) -> u16 {
        self.sock.local_addr().map_or(self.cfg.port, |a| a.port())
    }

    pub fn peers(&self) -> &[Peer] {
        &self.peers
    }

    /// Players heard from (they have an id).
    pub fn players(&self) -> impl Iterator<Item = (u8, SocketAddr)> + '_ {
        self.peers.iter().filter_map(|p| p.id.filter(|_| p.heard()).map(|id| (id, p.addr)))
    }

    /// Datagrams sent so far (diagnostics).
    pub fn sent_datagrams(&self) -> u64 {
        self.sent_datagrams
    }

    /// Adds an address to contact.
    pub fn join(&mut self, addr: SocketAddr, now: Instant) {
        if !self.peers.iter().any(|p| p.addr == addr) {
            self.peers.push(Peer {
                addr,
                id: None,
                nonce: 0,
                added: now,
                last_heard: None,
                last_seq: None,
                joined: true,
            });
        }
        // Say hello at once.
        self.last_beacon = None;
    }

    fn header(&self, kind: Kind, out: &mut Vec<u8>) {
        out.extend_from_slice(&MAGIC);
        out.push(PROTOCOL);
        out.push(kind as u8);
        out.push(self.cfg.game_kind);
        out.push(self.id);
        out.extend_from_slice(&self.cfg.release.to_le_bytes());
        out.extend_from_slice(&self.nonce.to_le_bytes());
    }

    fn lost(&mut self) -> bool {
        if self.cfg.loss_percent == 0 {
            return false;
        }
        // xorshift32
        let mut x = self.loss;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.loss = x;
        x % 100 < u32::from(self.cfg.loss_percent)
    }

    fn send_to(&mut self, buf: &[u8], addr: SocketAddr) {
        if self.lost() {
            return;
        }
        self.sent_datagrams += 1;
        // Unreachable peers are dealt with by the time-out.
        let _ = self.sock.send_to(buf, addr);
    }

    fn broadcast_addr(&self) -> SocketAddr {
        SocketAddr::new(IpAddr::V4(Ipv4Addr::BROADCAST), self.cfg.port)
    }

    fn send_all(&mut self, buf: &[u8], broadcast: bool) {
        let addrs: Vec<SocketAddr> = self.peers.iter().map(|p| p.addr).collect();
        for a in addrs {
            self.send_to(buf, a);
        }
        if broadcast && self.cfg.broadcast {
            let b = self.broadcast_addr();
            self.send_to(buf, b);
        }
    }

    fn state_datagram(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(HEADER + 1 + REDUNDANCY * (4 + RECORD));
        self.header(Kind::State, &mut out);
        out.push(self.history.len() as u8);
        for (seq, rec) in self.history.iter().rev() {
            out.extend_from_slice(&seq.to_le_bytes());
            out.extend_from_slice(rec);
        }
        out
    }

    /// Sends a record to every known player; `broadcast` also announces it
    /// on the LAN.
    pub fn send(&mut self, record: Record, broadcast: bool, now: Instant) {
        self.seq = self.seq.wrapping_add(1);
        self.history.push_back((self.seq, record));
        while self.history.len() > REDUNDANCY {
            self.history.pop_front();
        }
        let buf = self.state_datagram();
        self.send_all(&buf, broadcast);
        self.last_send = Some(now);
        self.tail = 0;
    }

    /// Sends a batch of records from one frame, merging identical ones (the
    /// original could send the same packet by broadcast and by unicast).
    pub fn send_frame(&mut self, records: &[(Record, bool)], now: Instant) {
        let mut merged: Vec<(Record, bool)> = Vec::new();
        for &(r, b) in records {
            match merged.iter_mut().find(|(m, _)| *m == r) {
                Some(m) => m.1 |= b,
                None => merged.push((r, b)),
            }
        }
        for (r, b) in merged {
            self.send(r, b, now);
        }
    }

    /// Says goodbye to everyone.
    pub fn leave(&mut self) {
        let mut out = Vec::with_capacity(HEADER);
        self.header(Kind::Bye, &mut out);
        self.send_all(&out, false);
    }

    fn parse_header(buf: &[u8]) -> Option<(Header, &[u8])> {
        if buf.len() < HEADER || buf[0..4] != MAGIC || buf[4] != PROTOCOL {
            return None;
        }
        let kind = Kind::from_u8(buf[5])?;
        let h = Header {
            kind,
            game_kind: buf[6],
            id: buf[7],
            release: u64::from_le_bytes(buf[8..16].try_into().ok()?),
            nonce: u64::from_le_bytes(buf[16..24].try_into().ok()?),
        };
        Some((h, &buf[HEADER..]))
    }

    fn peer_index(&mut self, addr: SocketAddr, now: Instant) -> Option<usize> {
        if let Some(i) = self.peers.iter().position(|p| p.addr == addr) {
            return Some(i);
        }
        let live = self.peers.iter().filter(|p| p.heard() || p.joined).count();
        if live >= self.cfg.max_peers {
            return None;
        }
        self.peers.push(Peer {
            addr,
            id: None,
            nonce: 0,
            added: now,
            last_heard: None,
            last_seq: None,
            joined: false,
        });
        Some(self.peers.len() - 1)
    }

    /// A free id, preferring the low byte of the nonce.
    fn free_id(&self) -> u8 {
        let used: Vec<u8> = self.peers.iter().filter_map(|p| p.id).collect();
        let start = (self.nonce >> 8) % 254;
        (0..254u64).map(|k| ((start + k) % 254) as u8 + 1).find(|i| !used.contains(i)).unwrap_or(1)
    }

    /// Receives everything waiting and runs the timers. Call every frame.
    pub fn poll(&mut self, now: Instant) -> Vec<Event> {
        let mut events = Vec::new();
        let mut buf = [0u8; 1500];
        loop {
            match self.sock.recv_from(&mut buf) {
                Ok((n, from)) => self.handle(&buf[..n], from, now, &mut events),
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => break,
                // Windows reports an earlier send to a closed port this way.
                Err(e) if e.kind() == io::ErrorKind::ConnectionReset => continue,
                Err(_) => break,
            }
        }
        self.timers(now, &mut events);
        events
    }

    fn handle(&mut self, buf: &[u8], from: SocketAddr, now: Instant, events: &mut Vec<Event>) {
        let Some((h, body)) = Self::parse_header(buf) else { return };
        if h.nonce == self.nonce {
            // Our own broadcast, or our own address learned by gossip.
            self.peers.retain(|p| p.addr != from);
            if !self.self_addrs.contains(&from) {
                self.self_addrs.push(from);
            }
            return;
        }
        if h.release != self.cfg.release || h.game_kind != self.cfg.game_kind {
            if !self.mismatched.contains(&from) {
                self.mismatched.push(from);
                events.push(Event::Mismatch(from));
            }
            return;
        }
        if h.id == 0 || h.id == 255 {
            return;
        }
        let Some(i) = self.peer_index(from, now) else { return };
        // Id collision: the machine with the larger nonce takes a new id.
        if h.id == self.id && h.nonce != self.nonce && self.nonce > h.nonce {
            self.peers[i].id = Some(h.id);
            self.id = self.free_id();
            events.push(Event::IdChanged(self.id));
            self.last_beacon = None;
        }
        let p = &mut self.peers[i];
        if p.nonce != h.nonce {
            // A new machine at this address (or a restart): start over.
            p.nonce = h.nonce;
            p.last_seq = None;
            p.last_heard = None;
        }
        let first = p.last_heard.is_none() || p.id != Some(h.id);
        let old_id = p.id;
        p.id = Some(h.id);
        p.last_heard = Some(now);
        if first {
            if let Some(o) = old_id.filter(|&o| o != h.id) {
                events.push(Event::Left(o));
            }
            events.push(Event::Joined { id: h.id, addr: from });
            // Answer at once so the newcomer learns about us too.
            let mut out = Vec::with_capacity(HEADER);
            self.header(Kind::Hello, &mut out);
            self.send_to(&out, from);
        }
        match h.kind {
            Kind::Hello => {}
            Kind::Bye => {
                events.push(Event::Left(h.id));
                self.peers.remove(i);
            }
            Kind::State => {
                let Some((&n, rest)) = body.split_first() else { return };
                let n = usize::from(n).min(REDUNDANCY);
                if rest.len() < n * (4 + RECORD) {
                    return;
                }
                let mut recs: Vec<(u32, Record)> = rest
                    .chunks_exact(4 + RECORD)
                    .take(n)
                    .map(|c| {
                        let seq = u32::from_le_bytes([c[0], c[1], c[2], c[3]]);
                        let mut r = [0u8; RECORD];
                        r.copy_from_slice(&c[4..]);
                        (seq, r)
                    })
                    .collect();
                recs.sort_by(|a, b| {
                    if seq_newer(a.0, b.0) {
                        std::cmp::Ordering::Greater
                    } else if a.0 == b.0 {
                        std::cmp::Ordering::Equal
                    } else {
                        std::cmp::Ordering::Less
                    }
                });
                let p = &mut self.peers[i];
                for (seq, r) in recs {
                    if p.last_seq.is_none_or(|l| seq_newer(seq, l)) {
                        p.last_seq = Some(seq);
                        events.push(Event::Record(r));
                    }
                }
            }
            Kind::Peers => {
                let Some((&n, mut rest)) = body.split_first() else { return };
                for _ in 0..usize::from(n).min(MAX_GOSSIP) {
                    let Some((addr, r)) = decode_addr(rest) else { break };
                    rest = r;
                    if !self.peers.iter().any(|p| p.addr == addr)
                        && !self.self_addrs.contains(&addr)
                        && !addr.ip().is_unspecified()
                    {
                        let _ = self.peer_index(addr, now);
                        self.last_beacon = None;
                    }
                }
            }
        }
    }

    fn timers(&mut self, now: Instant, events: &mut Vec<Event>) {
        // Time-outs.
        let mut i = 0;
        while i < self.peers.len() {
            let p = &self.peers[i];
            let since = p.last_heard.unwrap_or(p.added);
            if now.saturating_duration_since(since) > TIMEOUT {
                if let Some(id) = p.id.filter(|_| p.heard()) {
                    events.push(Event::Left(id));
                }
                if p.joined {
                    let p = &mut self.peers[i];
                    p.last_heard = None;
                    p.last_seq = None;
                    p.id = None;
                    p.added = now;
                } else {
                    self.peers.remove(i);
                    continue;
                }
            }
            i += 1;
        }
        // Delayed resends of the newest record.
        if let Some(t) = self.last_send
            && self.tail < TAIL.len()
            && now.saturating_duration_since(t) >= TAIL[self.tail]
        {
            self.tail += 1;
            let buf = self.state_datagram();
            self.send_all(&buf, false);
        }
        // Beacons: announce ourselves on the LAN and to everyone known.
        if self.last_beacon.is_none_or(|t| now.saturating_duration_since(t) >= BEACON) {
            self.last_beacon = Some(now);
            let mut out = Vec::with_capacity(HEADER);
            self.header(Kind::Hello, &mut out);
            self.send_all(&out, true);
        }
        // Gossip the players we have heard.
        if self.last_gossip.is_none_or(|t| now.saturating_duration_since(t) >= GOSSIP) {
            self.last_gossip = Some(now);
            let heard: Vec<SocketAddr> =
                self.peers.iter().filter(|p| p.heard()).map(|p| p.addr).take(MAX_GOSSIP).collect();
            if heard.len() > 1 {
                let mut out = Vec::new();
                self.header(Kind::Peers, &mut out);
                out.push(heard.len() as u8);
                for a in &heard {
                    encode_addr(*a, &mut out);
                }
                for a in heard {
                    self.send_to(&out.clone(), a);
                }
            }
        }
    }
}

impl Drop for Transport {
    fn drop(&mut self) {
        self.leave();
    }
}

fn encode_addr(a: SocketAddr, out: &mut Vec<u8>) {
    match a.ip() {
        IpAddr::V4(ip) => {
            out.push(4);
            out.extend_from_slice(&ip.octets());
        }
        IpAddr::V6(ip) => {
            out.push(6);
            out.extend_from_slice(&ip.octets());
        }
    }
    out.extend_from_slice(&a.port().to_le_bytes());
}

fn decode_addr(b: &[u8]) -> Option<(SocketAddr, &[u8])> {
    let (&fam, rest) = b.split_first()?;
    let (ip, rest) = match fam {
        4 if rest.len() >= 6 => {
            let o: [u8; 4] = rest[..4].try_into().ok()?;
            (IpAddr::V4(Ipv4Addr::from(o)), &rest[4..])
        }
        6 if rest.len() >= 18 => {
            let o: [u8; 16] = rest[..16].try_into().ok()?;
            (IpAddr::V6(Ipv6Addr::from(o)), &rest[16..])
        }
        _ => return None,
    };
    let port = u16::from_le_bytes([rest[0], rest[1]]);
    Some((SocketAddr::new(ip, port), &rest[2..]))
}

#[cfg(test)]
mod tests;
