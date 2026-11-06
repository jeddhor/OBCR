//! Loopback tests: real UDP sockets on 127.0.0.1, broadcast off.

use super::*;

const LOCAL: Ipv4Addr = Ipv4Addr::LOCALHOST;

fn cfg(id: Option<u8>) -> Config {
    Config { port: 0, release: release_id("OBC-A"), broadcast: false, id, ..Config::default() }
}

fn addr(t: &Transport) -> SocketAddr {
    SocketAddr::new(IpAddr::V4(LOCAL), t.port())
}

fn rec(n: u8) -> Record {
    let mut r = [0u8; RECORD];
    r[0] = n;
    r[2] = n;
    r
}

/// Polls everyone a few rounds, advancing the clock, and collects events.
fn pump(ts: &mut [&mut Transport], now: &mut Instant, rounds: u32) -> Vec<Vec<Event>> {
    let mut out = vec![Vec::new(); ts.len()];
    for _ in 0..rounds {
        *now += Duration::from_millis(30);
        std::thread::sleep(Duration::from_millis(2));
        for (i, t) in ts.iter_mut().enumerate() {
            out[i].extend(t.poll(*now));
        }
    }
    out
}

fn records(ev: &[Event]) -> Vec<Record> {
    ev.iter().filter_map(|e| if let Event::Record(r) = e { Some(*r) } else { None }).collect()
}

#[test]
fn two_players_meet_and_exchange_records() {
    let mut now = Instant::now();
    let mut a = Transport::open(cfg(Some(10)), now).unwrap();
    let mut b = Transport::open(Config { join: vec![addr(&a)], ..cfg(Some(20)) }, now).unwrap();
    let ev = pump(&mut [&mut a, &mut b], &mut now, 5);
    assert!(ev[0].iter().any(|e| matches!(e, Event::Joined { id: 20, .. })), "{:?}", ev[0]);
    assert!(ev[1].iter().any(|e| matches!(e, Event::Joined { id: 10, .. })), "{:?}", ev[1]);
    for n in 1..=5 {
        a.send(rec(n), false, now);
    }
    let ev = pump(&mut [&mut a, &mut b], &mut now, 5);
    assert_eq!(records(&ev[1]), (1..=5).map(rec).collect::<Vec<_>>(), "in order, once each");
}

#[test]
fn redundancy_recovers_losses_without_duplicates() {
    let mut now = Instant::now();
    let mut a = Transport::open(Config { loss_percent: 30, ..cfg(Some(10)) }, now).unwrap();
    let mut b = Transport::open(Config { join: vec![addr(&a)], ..cfg(Some(20)) }, now).unwrap();
    pump(&mut [&mut a, &mut b], &mut now, 10);
    let mut got = Vec::new();
    for n in 1..=200u8 {
        a.send(rec(n), false, now);
        got.extend(records(&pump(&mut [&mut a, &mut b], &mut now, 1)[1]));
    }
    got.extend(records(&pump(&mut [&mut a, &mut b], &mut now, 20)[1]));
    let mut sorted = got.clone();
    sorted.sort();
    sorted.dedup();
    assert_eq!(sorted.len(), got.len(), "no duplicates");
    assert!(got.windows(2).all(|w| w[0][0] < w[1][0]), "in order");
    // 30 % loss per datagram, each record in three datagrams plus resends:
    // nearly all arrive.
    assert!(got.len() >= 190, "{} of 200 arrived", got.len());
}

#[test]
fn colliding_ids_are_resolved() {
    let mut now = Instant::now();
    let mut a = Transport::open(cfg(Some(7)), now).unwrap();
    let mut b = Transport::open(Config { join: vec![addr(&a)], ..cfg(Some(7)) }, now).unwrap();
    let ev = pump(&mut [&mut a, &mut b], &mut now, 10);
    assert_ne!(a.id(), b.id());
    let changed = ev.iter().flatten().filter(|e| matches!(e, Event::IdChanged(_))).count();
    assert_eq!(changed, 1, "exactly one machine moved: {ev:?}");
}

#[test]
fn gossip_connects_everyone() {
    let mut now = Instant::now();
    let mut hub = Transport::open(cfg(Some(1)), now).unwrap();
    let mut a = Transport::open(Config { join: vec![addr(&hub)], ..cfg(Some(2)) }, now).unwrap();
    let mut b = Transport::open(Config { join: vec![addr(&hub)], ..cfg(Some(3)) }, now).unwrap();
    pump(&mut [&mut hub, &mut a, &mut b], &mut now, 150);
    let ids = |t: &Transport| {
        let mut v: Vec<u8> = t.players().map(|(id, _)| id).collect();
        v.sort();
        v
    };
    assert_eq!(ids(&a), vec![1, 3]);
    assert_eq!(ids(&b), vec![1, 2]);
    assert_eq!(ids(&hub), vec![2, 3]);
}

#[test]
fn goodbye_and_timeout_report_leaving() {
    let mut now = Instant::now();
    let mut a = Transport::open(cfg(Some(10)), now).unwrap();
    let mut b = Transport::open(Config { join: vec![addr(&a)], ..cfg(Some(20)) }, now).unwrap();
    let mut c = Transport::open(Config { join: vec![addr(&a)], ..cfg(Some(30)) }, now).unwrap();
    pump(&mut [&mut a, &mut b, &mut c], &mut now, 5);
    drop(b);
    let ev = pump(&mut [&mut a, &mut c], &mut now, 3);
    assert!(ev[0].contains(&Event::Left(20)), "{:?}", ev[0]);
    // c falls silent: a times it out.
    now += TIMEOUT + Duration::from_secs(1);
    let ev = a.poll(now);
    assert!(ev.contains(&Event::Left(30)), "{ev:?}");
}

#[test]
fn mismatched_releases_ignore_each_other() {
    let mut now = Instant::now();
    let mut a = Transport::open(cfg(Some(10)), now).unwrap();
    let mut b = Transport::open(
        Config { join: vec![addr(&a)], release: release_id("OBC-B"), ..cfg(Some(20)) },
        now,
    )
    .unwrap();
    let ev = pump(&mut [&mut a, &mut b], &mut now, 5);
    assert!(ev[0].iter().any(|e| matches!(e, Event::Mismatch(_))));
    assert!(!ev[0].iter().any(|e| matches!(e, Event::Joined { .. })));
}

#[test]
fn send_frame_merges_identical_records() {
    let mut now = Instant::now();
    let mut a = Transport::open(cfg(Some(10)), now).unwrap();
    let mut b = Transport::open(Config { join: vec![addr(&a)], ..cfg(Some(20)) }, now).unwrap();
    pump(&mut [&mut a, &mut b], &mut now, 5);
    a.send_frame(&[(rec(9), true), (rec(9), false)], now);
    let ev = pump(&mut [&mut a, &mut b], &mut now, 5);
    assert_eq!(records(&ev[1]), vec![rec(9)]);
}

#[test]
fn addresses_parse() {
    assert_eq!(resolve("10.0.0.2", 16387).unwrap(), "10.0.0.2:16387".parse().unwrap());
    assert_eq!(resolve("10.0.0.2:999", 16387).unwrap(), "10.0.0.2:999".parse().unwrap());
    assert_eq!(resolve("[::1]:5", 1).unwrap(), "[::1]:5".parse().unwrap());
    assert!(resolve("localhost", 16387).unwrap().ip().is_loopback());
    let mut v = Vec::new();
    encode_addr("[::1]:5".parse().unwrap(), &mut v);
    encode_addr("1.2.3.4:6".parse().unwrap(), &mut v);
    let (x, rest) = decode_addr(&v).unwrap();
    let (y, rest) = decode_addr(rest).unwrap();
    assert_eq!((x, y), ("[::1]:5".parse().unwrap(), "1.2.3.4:6".parse().unwrap()));
    assert!(rest.is_empty());
}
