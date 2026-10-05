use std::{
    collections::HashMap,
    hash::{BuildHasher, RandomState},
    marker::PhantomData,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
    vec::Drain,
};

use crate::net::{
    Transport,
    codec::{Message, Scratch},
    conn::{Channel, Connection, Params, SendError},
    wire::{
        KIND_ACCEPT, KIND_CHALLENGE, KIND_CONNECT, KIND_DATA, KIND_DISCONNECT, KIND_MASK,
        KIND_RESPONSE, MIN_CONNECT, Reader,
    },
};

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, PartialOrd, Ord)]
pub struct ConnId(pub u32);

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DisconnectReason {
    Local,
    Remote,
    TimedOut,
    ConnectFailed,
}

#[derive(Debug)]
pub enum Event<R> {
    Connected(ConnId),
    Message(ConnId, R),
    Disconnected(ConnId, DisconnectReason),
}

#[derive(Clone, Debug)]
pub struct Config {
    pub accept: bool,
    pub max_connections: usize,
    pub keepalive: Duration,
    pub timeout: Duration,
    pub connect_timeout: Duration,
    pub connect_retry: Duration,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            accept: true,
            max_connections: 64,
            keepalive: Duration::from_secs(1),
            timeout: Duration::from_secs(10),
            connect_timeout: Duration::from_secs(5),
            connect_retry: Duration::from_millis(250),
        }
    }
}

enum Phase {
    AwaitChallenge,
    AwaitAccept(u64),
    Open,
}

struct Peer<A> {
    addr: A,
    salt: u64,
    phase: Phase,
    conn: Connection,
    started: Instant,
    last_try: Instant,
}

const MAX_RX_PER_POLL: usize = 4096;
const COOKIE_EPOCH_SECS: u64 = 8;

pub struct Endpoint<T: Transport, S: Message, R: Message> {
    t: T,
    cfg: Config,
    mtu: usize,
    peers: HashMap<ConnId, Peer<T::Addr>>,
    by_addr: HashMap<T::Addr, ConnId>,
    next_id: u32,
    events: Vec<Event<R>>,
    rx: Vec<u8>,
    tx: Vec<u8>,
    scratch: Scratch,
    keys: RandomState,
    ctr: u64,
    epoch0: Instant,
    _s: PhantomData<fn(S)>,
}

impl<T: Transport, S: Message, R: Message> Endpoint<T, S, R> {
    pub fn new(transport: T, cfg: Config) -> Self {
        let mtu = transport.max_mtu();
        Self {
            t: transport,
            cfg,
            mtu,
            peers: HashMap::new(),
            by_addr: HashMap::new(),
            next_id: 1,
            events: Vec::new(),
            rx: vec![0; mtu.max(MIN_CONNECT) + 64],
            tx: vec![0; mtu.max(MIN_CONNECT)],
            scratch: Scratch::default(),
            keys: RandomState::new(),
            ctr: 0,
            epoch0: Instant::now(),
            _s: PhantomData,
        }
    }

    pub fn transport(&self) -> &T {
        &self.t
    }
    pub fn transport_mut(&mut self) -> &mut T {
        &mut self.t
    }

    pub fn connect(&mut self, addr: T::Addr, now: Instant) -> ConnId {
        if let Some(&id) = self.by_addr.get(&addr) {
            return id;
        }
        let salt = self.rand64();
        let id = self.alloc_id();
        let peer = Peer {
            addr,
            salt,
            phase: Phase::AwaitChallenge,
            conn: self.new_conn(addr, now),
            started: now,
            last_try: now,
        };
        self.peers.insert(id, peer);
        self.by_addr.insert(addr, id);
        self.send_connect(addr, salt);
        id
    }
    pub fn send(&mut self, id: ConnId, ch: Channel, msg: &S) -> Result<(), SendError> {
        let bytes = msg
            .encode(&mut self.scratch)
            .map_err(|_| SendError::Encode)?;
        let peer = self
            .peers
            .get_mut(&id)
            .ok_or(SendError::UnknownConnection)?;
        if !matches!(peer.phase, Phase::Open) {
            return Err(SendError::NotConnected);
        }
        peer.conn.send(ch, bytes)
    }
    pub fn broadcast(&mut self, ch: Channel, msg: &S) -> Result<(), SendError> {
        let bytes = msg
            .encode(&mut self.scratch)
            .map_err(|_| SendError::Encode)?;
        let mut res = Ok(());
        for peer in self
            .peers
            .values_mut()
            .filter(|p| matches!(p.phase, Phase::Open))
        {
            if let Err(e) = peer.conn.send(ch, bytes) {
                res = res.and(Err(e))
            }
        }
        res
    }
    pub fn disconnect(&mut self, id: ConnId) {
        if let Some(peer) = self.peers.remove(&id) {
            self.by_addr.remove(&peer.addr);
            self.send_disconnect(peer.addr, peer.salt);
            self.events
                .push(Event::Disconnected(id, DisconnectReason::Local));
        }
    }

    pub fn rtt(&self, id: ConnId) -> Option<Duration> {
        self.peers.get(&id)?.conn.rtt()
    }
    pub fn connections(&self) -> impl Iterator<Item = ConnId> + '_ {
        self.peers
            .iter()
            .filter(|(_, p)| matches!(p.phase, Phase::Open))
            .map(|(id, _)| *id)
    }
    pub fn next_timeout(&self, now: Instant) -> Instant {
        let mut t = now + self.cfg.keepalive;
        for p in self.peers.values() {
            let pt = match p.phase {
                Phase::Open => p.conn.next_timeout(now),
                _ => p.last_try + self.cfg.connect_retry,
            };
            t = t.min(pt);
        }
        t.max(now)
    }

    pub fn poll(&mut self, now: Instant) -> Drain<'_, Event<R>> {
        let mut rx = std::mem::take(&mut self.rx);
        for _ in 0..MAX_RX_PER_POLL {
            match self.t.recv(&mut rx) {
                Ok(Some((addr, n))) => {
                    let n = n.min(rx.len());
                    self.on_datagram(addr, &rx[..n], now);
                }
                Ok(None) | Err(_) => break,
            }
        }
        self.rx = rx;

        let mut dead: Vec<(ConnId, DisconnectReason)> = Vec::new();
        let (cfg, t, tx) = (&self.cfg, &mut self.t, &mut self.tx);
        for (&id, peer) in self.peers.iter_mut() {
            match peer.phase {
                Phase::Open => {
                    if peer.conn.timed_out(now) {
                        dead.push((id, DisconnectReason::TimedOut));
                        continue;
                    }
                    let addr = peer.addr;
                    peer.conn.flush(now, tx, &mut |pkt| {
                        let _ = t.send(addr, pkt);
                    });
                }
                Phase::AwaitChallenge | Phase::AwaitAccept(_) => {
                    if now.saturating_duration_since(peer.started) >= cfg.connect_timeout {
                        dead.push((id, DisconnectReason::ConnectFailed));
                    } else if now.saturating_duration_since(peer.last_try) >= cfg.connect_retry {
                        peer.last_try = now;
                        match peer.phase {
                            Phase::AwaitAccept(cookie) => {
                                let mut b = [0u8; 17];
                                b[0] = KIND_RESPONSE;
                                b[1..9].copy_from_slice(&peer.salt.to_le_bytes());
                                b[9..17].copy_from_slice(&cookie.to_le_bytes());
                                let _ = t.send(peer.addr, &b);
                            }
                            _ => {
                                let mut b = [0u8; MIN_CONNECT];
                                b[0] = KIND_CONNECT;
                                b[1..9].copy_from_slice(&peer.salt.to_le_bytes());
                                let _ = t.send(peer.addr, &b);
                            }
                        }
                    }
                }
            }
        }
        for (id, reason) in dead {
            if let Some(peer) = self.peers.remove(&id) {
                self.by_addr.remove(&peer.addr);
                if reason == DisconnectReason::TimedOut {
                    self.send_disconnect(peer.addr, peer.salt);
                }
                self.events.push(Event::Disconnected(id, reason));
            }
        }
        self.events.drain(..)
    }

    // --------------------------------------------------- //
    // Internal helpers                                    //
    // --------------------------------------------------- //

    fn new_conn(&self, addr: T::Addr, now: Instant) -> Connection {
        Connection::new(
            Params {
                mtu: self.mtu,
                passthrough: self.t.is_reliable(addr),
                keepalive: self.cfg.keepalive,
                timeout: self.cfg.timeout,
            },
            now,
        )
    }

    fn alloc_id(&mut self) -> ConnId {
        let id = ConnId(self.next_id);
        self.next_id = self.next_id.wrapping_add(1).max(1);
        id
    }
    fn rand64(&mut self) -> u64 {
        self.ctr += 1;
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.subsec_nanos());
        self.keys.hash_one((self.ctr, nanos))
    }
    fn epoch(&self, now: Instant) -> u64 {
        now.saturating_duration_since(self.epoch0).as_secs() / COOKIE_EPOCH_SECS
    }
    fn cookie(&self, addr: T::Addr, salt: u64, epoch: u64) -> u64 {
        self.keys.hash_one((addr, salt, epoch))
    }

    fn send_connect(&mut self, addr: T::Addr, salt: u64) {
        let mut b = [0u8; MIN_CONNECT];
        b[0] = KIND_CONNECT;
        b[1..9].copy_from_slice(&salt.to_le_bytes());
        let _ = self.t.send(addr, &b);
    }
    fn send_disconnect(&mut self, addr: T::Addr, salt: u64) {
        let mut b = [0u8; 9];
        b[0] = KIND_DISCONNECT;
        b[1..9].copy_from_slice(&salt.to_le_bytes());
        let _ = self.t.send(addr, &b);
    }

    fn on_datagram(&mut self, addr: T::Addr, data: &[u8], now: Instant) {
        let Some(&b0) = data.first() else { return };
        match b0 & KIND_MASK {
            KIND_DATA => {
                let Some(&id) = self.by_addr.get(&addr) else {
                    self.send_disconnect(addr, 0);
                    return;
                };
                let events = &mut self.events;
                let Some(peer) = self.peers.get_mut(&id) else {
                    return;
                };
                if matches!(peer.phase, Phase::Open) {
                    peer.conn.handle_datagram(now, data, &mut |payload| {
                        if let Ok(m) = R::decode(payload) {
                            events.push(Event::Message(id, m));
                        }
                    });
                }
            }
            KIND_CONNECT => {
                if !self.cfg.accept || data.len() < MIN_CONNECT {
                    return;
                }
                let mut r = Reader::new(&data[1..]);
                let Some(salt) = r.u64() else { return };
                let cookie = self.cookie(addr, salt, self.epoch(now));
                let mut b = [0u8; 17];
                b[0] = KIND_CHALLENGE;
                b[1..9].copy_from_slice(&salt.to_le_bytes());
                b[9..17].copy_from_slice(&cookie.to_le_bytes());
                let _ = self.t.send(addr, &b);
            }
            KIND_CHALLENGE => {
                let mut r = Reader::new(&data[1..]);
                let (Some(salt), Some(cookie)) = (r.u64(), r.u64()) else {
                    return;
                };
                let Some(&id) = self.by_addr.get(&addr) else {
                    return;
                };
                let Some(peer) = self.peers.get_mut(&id) else {
                    return;
                };
                if peer.salt != salt || matches!(peer.phase, Phase::Open) {
                    return;
                }
                peer.phase = Phase::AwaitAccept(cookie);
                peer.last_try = now;
                let mut b = [0u8; 17];
                b[0] = KIND_RESPONSE;
                b[1..9].copy_from_slice(&salt.to_le_bytes());
                b[9..17].copy_from_slice(&cookie.to_le_bytes());
                let _ = self.t.send(addr, &b);
            }
            KIND_RESPONSE => {
                if !self.cfg.accept {
                    return;
                }
                let mut r = Reader::new(&data[1..]);
                let (Some(salt), Some(cookie)) = (r.u64(), r.u64()) else {
                    return;
                };
                let e = self.epoch(now);
                let ok = cookie == self.cookie(addr, salt, e)
                    || (e > 0 && cookie == self.cookie(addr, salt, e - 1));
                if !ok {
                    return;
                }
                match self.by_addr.get(&addr).copied() {
                    Some(id) if self.peers.get(&id).is_some_and(|p| p.salt == salt) => {}
                    existing => {
                        if let Some(old) = existing {
                            if let Some(p) = self.peers.remove(&old) {
                                self.by_addr.remove(&p.addr);
                                self.events
                                    .push(Event::Disconnected(old, DisconnectReason::Remote));
                            }
                        } else if self.peers.len() >= self.cfg.max_connections {
                            self.send_disconnect(addr, salt);
                            return;
                        }
                        let id = self.alloc_id();
                        let peer = Peer {
                            addr,
                            salt,
                            phase: Phase::Open,
                            conn: self.new_conn(addr, now),
                            started: now,
                            last_try: now,
                        };
                        self.peers.insert(id, peer);
                        self.by_addr.insert(addr, id);
                        self.events.push(Event::Connected(id));
                    }
                }
                let mut b = [0u8; 9];
                b[0] = KIND_ACCEPT;
                b[1..9].copy_from_slice(&salt.to_le_bytes());
                let _ = self.t.send(addr, &b);
            }
            KIND_ACCEPT => {
                let mut r = Reader::new(&data[1..]);
                let Some(salt) = r.u64() else { return };
                let Some(&id) = self.by_addr.get(&addr) else {
                    return;
                };
                let Some(peer) = self.peers.get_mut(&id) else {
                    return;
                };
                if peer.salt == salt && matches!(peer.phase, Phase::AwaitAccept(_)) {
                    peer.phase = Phase::Open;
                    peer.conn
                        .seed_rtt(now.saturating_duration_since(peer.last_try));
                    self.events.push(Event::Connected(id));
                }
            }
            KIND_DISCONNECT => {
                let mut r = Reader::new(&data[1..]);
                let Some(salt) = r.u64() else { return };
                let Some(&id) = self.by_addr.get(&addr) else {
                    return;
                };
                let ok = self
                    .peers
                    .get(&id)
                    .is_some_and(|p| p.salt == salt || (salt == 0 && !self.cfg.accept));
                if ok {
                    self.peers.remove(&id);
                    self.by_addr.remove(&addr);
                    self.events
                        .push(Event::Disconnected(id, DisconnectReason::Remote));
                }
            }
            _ => {}
        }
    }
}
