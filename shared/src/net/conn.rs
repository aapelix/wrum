use std::{
    collections::{HashMap, VecDeque},
    time::{Duration, Instant},
};

use crate::net::{
    window::{Insert, Window},
    wire::{CH_MASK, FRAG, HAS_ACK, HAS_SEQ, KIND_DATA, LAST, Reader, put_varint, varint_len},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Channel {
    ReliableOrdered = 0,
    ReliableUnordered = 1,
    Unreliable = 2,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SendError {
    UnknownConnection,
    NotConnected,
    TooLarge,
    QueueFull,
    Encode,
}
impl std::fmt::Display for SendError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for SendError {}

pub const WINDOW: usize = 1024;
const MAX_FRAGS: usize = 255;
const MAX_PARTIALS: usize = 16;
const MAX_UNRELIABLE_Q: usize = 4096;
const HDR_MAX: usize = 1 + 2 + 6;
const MSG_OVERHEAD: usize = 1 + 2 + 2 + 3;
const MIN_RTO: Duration = Duration::from_millis(50);
const MAX_RTO: Duration = Duration::from_secs(2);
const MAX_BACKOFF_SHIFT: u8 = 4;

pub struct Params {
    pub mtu: usize,
    /// transport is already reliable+ordered
    pub passthrough: bool,
    pub keepalive: Duration,
    pub timeout: Duration,
}

struct Rel {
    id: u64,
    chan: u8,
    seq: u16,
    frag: Option<(u8, u8)>,
    data: Vec<u8>,
    len: usize,
    sent_at: Option<Instant>,
    tries: u8,
    resend_now: bool,
    acked: bool,
}
struct SentPacket {
    seq: u16,
    at: Instant,
    ids: Vec<u64>,
}
struct Pending {
    frag: Option<(u8, u8)>,
    data: Vec<u8>,
}
struct Partial {
    count: u8,
    got: u8,
    parts: Vec<Option<Vec<u8>>>,
}

pub struct Connection {
    p: Params,

    next_id: u64,
    base_id: u64,
    unacked: VecDeque<Rel>,
    next_seq: [u16; 2],
    unreliable_q: VecDeque<Vec<u8>>,
    next_pkt_seq: u16,
    sent: VecDeque<SentPacket>,
    scan: usize,

    recv_latest: Option<u16>,
    recv_bits: u32,
    ack_dirty: bool,

    ordered: Window<Pending>,
    unordered: Window<()>,
    asm: [HashMap<u16, Partial>; 2],

    srtt: Option<Duration>,
    rttvar: Duration,
    rto: Duration,
    cwnd: usize,
    ssthresh: usize,
    in_flight: usize,
    recovery_until: Instant,

    last_recv: Instant,
    last_send: Instant,
}

#[inline]
fn backoff(rto: Duration, tries: u8) -> Duration {
    (rto * (1u32 << tries.min(MAX_BACKOFF_SHIFT))).min(MAX_RTO * 2)
}

impl Connection {
    pub fn new(p: Params, now: Instant) -> Self {
        let cwnd = 10 * p.mtu;
        Self {
            p,
            next_id: 0,
            base_id: 0,
            unacked: VecDeque::new(),
            next_seq: [0; 2],
            unreliable_q: VecDeque::new(),
            next_pkt_seq: 0,
            sent: VecDeque::new(),
            scan: 0,
            recv_latest: None,
            recv_bits: 0,
            ack_dirty: false,
            ordered: Window::new(WINDOW),
            unordered: Window::new(WINDOW),
            asm: [HashMap::new(), HashMap::new()],
            srtt: None,
            rttvar: Duration::ZERO,
            rto: Duration::from_millis(300),
            cwnd,
            ssthresh: usize::MAX,
            in_flight: 0,
            recovery_until: now,
            last_recv: now,
            last_send: now,
        }
    }

    pub fn max_chunk(&self) -> usize {
        self.p.mtu.saturating_sub(HDR_MAX + MSG_OVERHEAD)
    }

    pub fn rtt(&self) -> Option<Duration> {
        self.srtt
    }
    pub fn seed_rtt(&mut self, sample: Duration) {
        self.rtt_sample(sample);
    }

    pub fn timed_out(&self, now: Instant) -> bool {
        now.saturating_duration_since(self.last_recv) >= self.p.timeout
    }

    // ------------------------------------------------------------------ send

    pub fn send(&mut self, ch: Channel, payload: &[u8]) -> Result<(), SendError> {
        let ch = if self.p.passthrough {
            Channel::Unreliable
        } else {
            ch
        };
        let chunk = self.max_chunk();
        if ch == Channel::Unreliable {
            if payload.len() > chunk {
                return Err(SendError::TooLarge);
            }
            if self.unreliable_q.len() >= MAX_UNRELIABLE_Q {
                return Err(SendError::QueueFull);
            }
            self.unreliable_q.push_back(payload.to_vec());
            return Ok(());
        }

        let n = payload.len().div_ceil(chunk).max(1);
        if n > MAX_FRAGS {
            return Err(SendError::TooLarge);
        }
        if self.unacked.len() + n > WINDOW {
            return Err(SendError::QueueFull);
        }
        if n == 1 {
            self.push_rel(ch as u8, None, payload);
        } else {
            for (i, part) in payload.chunks(chunk).enumerate() {
                self.push_rel(ch as u8, Some((i as u8, n as u8)), part);
            }
        }
        Ok(())
    }

    fn push_rel(&mut self, chan: u8, frag: Option<(u8, u8)>, data: &[u8]) {
        let seq = self.next_seq[chan as usize];
        self.next_seq[chan as usize] = seq.wrapping_add(1);
        let id = self.next_id;
        self.next_id += 1;
        self.unacked.push_back(Rel {
            id,
            chan,
            seq,
            frag,
            data: data.to_vec(),
            len: data.len(),
            sent_at: None,
            tries: 0,
            resend_now: false,
            acked: false,
        });
    }

    pub fn next_timeout(&self, now: Instant) -> Instant {
        if self.ack_dirty || !self.unreliable_q.is_empty() {
            return now;
        }
        let mut t = (self.last_send + self.p.keepalive).min(self.last_recv + self.p.timeout);
        for r in self.unacked.iter().filter(|r| !r.acked) {
            match r.sent_at {
                None => {
                    if self.in_flight == 0 || self.in_flight + r.len <= self.cwnd {
                        return now;
                    }
                }
                Some(_) if r.resend_now => return now,
                Some(s) => t = t.min(s + backoff(self.rto, r.tries)),
            }
        }
        t.max(now)
    }
    pub fn flush(&mut self, now: Instant, buf: &mut [u8], sink: &mut dyn FnMut(&[u8])) {
        self.scan = 0;
        let mut ping = now.saturating_duration_since(self.last_send) >= self.p.keepalive;
        while let Some((s, e)) = self.build_packet(now, buf, ping) {
            ping = false;
            self.last_send = now;
            sink(&buf[s..e]);
        }
    }

    fn build_packet(&mut self, now: Instant, buf: &mut [u8], ping: bool) -> Option<(usize, usize)> {
        let mtu = self.p.mtu.min(buf.len());
        let mut pos = HDR_MAX;
        let mut n = 0usize;
        let mut ids: Vec<u64> = Vec::new();
        let mut last: Option<(usize, usize, usize)> = None;
        let mut timeout_loss = false;

        while self.scan < self.unacked.len() {
            let r = &mut self.unacked[self.scan];
            if r.acked {
                self.scan += 1;
                continue;
            }
            let timer_due = match r.sent_at {
                None => false,
                Some(t) => now >= t + backoff(self.rto, r.tries),
            };
            let due = r.sent_at.is_none() || r.resend_now || timer_due;
            if !due {
                self.scan += 1;
                continue;
            }
            if r.sent_at.is_none() && self.in_flight > 0 && self.in_flight + r.len > self.cwnd {
                self.scan = self.unacked.len();
                break;
            }
            let need =
                1 + 2 + if r.frag.is_some() { 2 } else { 0 } + varint_len(r.len as u32) + r.len;
            if pos + need > mtu {
                break;
            }
            let fp = pos;
            buf[pos] = r.chan | if r.frag.is_some() { FRAG } else { 0 };
            pos += 1;
            buf[pos..pos + 2].copy_from_slice(&r.seq.to_le_bytes());
            pos += 2;
            if let Some((i, c)) = r.frag {
                buf[pos] = i;
                buf[pos + 1] = c;
                pos += 2;
            }
            let lp = pos;
            put_varint(buf, &mut pos, r.len as u32);
            last = Some((fp, lp, pos - lp));
            buf[pos..pos + r.len].copy_from_slice(&r.data);
            pos += r.len;

            ids.push(r.id);
            n += 1;
            if r.sent_at.is_none() {
                self.in_flight += r.len;
            } else if timer_due && !r.resend_now {
                r.tries = r.tries.saturating_add(1);
                timeout_loss = true;
            }
            r.sent_at = Some(now);
            r.resend_now = false;
            self.scan += 1;
        }
        let has_rel = n > 0;

        while let Some(m) = self.unreliable_q.front() {
            let need = 1 + varint_len(m.len() as u32) + m.len();
            if pos + need > mtu {
                break;
            }
            let fp = pos;
            buf[pos] = Channel::Unreliable as u8;
            pos += 1;
            let lp = pos;
            put_varint(buf, &mut pos, m.len() as u32);
            last = Some((fp, lp, pos - lp));
            buf[pos..pos + m.len()].copy_from_slice(m);
            pos += m.len();
            n += 1;
            self.unreliable_q.pop_front();
        }

        if timeout_loss {
            self.on_loss(now);
        }
        if n == 0 && !self.ack_dirty && !ping {
            return None;
        }

        if let Some((fp, lp, ls)) = last {
            buf.copy_within(lp + ls..pos, lp);
            pos -= ls;
            buf[fp] |= LAST;
        }

        let want_seq = has_rel || ping;
        let ack = self.ack_dirty && self.recv_latest.is_some();
        let hlen = 1 + if want_seq { 2 } else { 0 } + if ack { 6 } else { 0 };
        let start = HDR_MAX - hlen;
        let mut hp = start;
        buf[hp] = KIND_DATA | if ack { HAS_ACK } else { 0 } | if want_seq { HAS_SEQ } else { 0 };
        hp += 1;
        if want_seq {
            let s = self.next_pkt_seq;
            self.next_pkt_seq = s.wrapping_add(1);
            buf[hp..hp + 2].copy_from_slice(&s.to_le_bytes());
            hp += 2;
            self.sent.push_back(SentPacket {
                seq: s,
                at: now,
                ids,
            });
            if self.sent.len() > 4 * WINDOW {
                self.sent.pop_front();
            }
        }
        if ack {
            buf[hp..hp + 2].copy_from_slice(&self.recv_latest.unwrap_or(0).to_le_bytes());
            buf[hp + 2..hp + 6].copy_from_slice(&self.recv_bits.to_le_bytes());
            self.ack_dirty = false;
        }
        Some((start, pos))
    }

    // --------------------------------------------------------------- receive

    pub fn handle_datagram(&mut self, now: Instant, data: &[u8], deliver: &mut dyn FnMut(&[u8])) {
        self.last_recv = now;
        let mut r = Reader::new(data);
        let Some(flags) = r.u8() else { return };
        let seq = if flags & HAS_SEQ != 0 {
            match r.u16() {
                Some(s) => Some(s),
                None => return,
            }
        } else {
            None
        };
        if flags & HAS_ACK != 0 {
            let (Some(a), Some(b)) = (r.u16(), r.u32()) else {
                return;
            };
            self.on_ack(now, a, b);
        }
        if let Some(s) = seq {
            self.note_recv(s);
        }

        while r.remaining() > 0 {
            let Some(mf) = r.u8() else { break };
            let ch = mf & CH_MASK;
            if ch > 2 {
                break;
            }
            let mseq = if ch < 2 {
                match r.u16() {
                    Some(s) => s,
                    None => break,
                }
            } else {
                0
            };
            let frag = if mf & FRAG != 0 {
                match (r.u8(), r.u8()) {
                    (Some(i), Some(c)) if c >= 1 && i < c => Some((i, c)),
                    _ => break,
                }
            } else {
                None
            };
            let len = if mf & LAST != 0 {
                r.remaining()
            } else {
                match r.varint() {
                    Some(l) => l as usize,
                    None => break,
                }
            };
            let Some(payload) = r.take(len) else { break };
            match ch {
                2 if frag.is_none() => deliver(payload),
                0 => self.recv_ordered(mseq, frag, payload, deliver),
                1 => self.recv_unordered(mseq, frag, payload, deliver),
                _ => break,
            }
            if mf & LAST != 0 {
                break;
            }
        }
    }

    fn note_recv(&mut self, seq: u16) {
        self.ack_dirty = true;
        let Some(latest) = self.recv_latest else {
            self.recv_latest = Some(seq);
            self.recv_bits = 0;
            return;
        };
        let d = seq.wrapping_sub(latest);
        if d == 0 {
            return;
        }
        if d < 0x8000 {
            self.recv_bits = if d <= 32 {
                (((self.recv_bits as u64) << d) | (1u64 << (d - 1))) as u32
            } else {
                0
            };
            self.recv_latest = Some(seq);
        } else {
            let back = latest.wrapping_sub(seq);
            if back <= 32 {
                self.recv_bits |= 1 << (back - 1);
            }
        }
    }
    fn recv_unordered(
        &mut self,
        seq: u16,
        frag: Option<(u8, u8)>,
        payload: &[u8],
        deliver: &mut dyn FnMut(&[u8]),
    ) {
        if self.unordered.insert(seq, ()) == Insert::Ok {
            while self.unordered.pop_ready().is_some() {}
            self.assemble(1, seq, frag, payload, deliver);
        }
    }
    fn recv_ordered(
        &mut self,
        seq: u16,
        frag: Option<(u8, u8)>,
        payload: &[u8],
        deliver: &mut dyn FnMut(&[u8]),
    ) {
        if seq == self.ordered.base() {
            self.assemble(0, seq, frag, payload, deliver);
            self.ordered.advance();
            while let Some((s, p)) = self.ordered.pop_ready() {
                self.assemble(0, s, p.frag, &p.data, deliver);
            }
        } else {
            let _ = self.ordered.insert(
                seq,
                Pending {
                    frag,
                    data: payload.to_vec(),
                },
            );
        }
    }
    fn assemble(
        &mut self,
        ch: usize,
        seq: u16,
        frag: Option<(u8, u8)>,
        payload: &[u8],
        deliver: &mut dyn FnMut(&[u8]),
    ) {
        let Some((idx, count)) = frag else {
            return deliver(payload);
        };
        let gid = seq.wrapping_sub(idx as u16);
        let map = &mut self.asm[ch];
        if !map.contains_key(&gid) {
            if map.len() >= MAX_PARTIALS {
                return;
            }
            map.insert(
                gid,
                Partial {
                    count,
                    got: 0,
                    parts: vec![None; count as usize],
                },
            );
        }
        let Some(p) = map.get_mut(&gid) else { return };
        if p.count != count || p.parts[idx as usize].is_some() {
            return;
        }
        p.parts[idx as usize] = Some(payload.to_vec());
        p.got += 1;
        if p.got == p.count {
            let Some(p) = map.remove(&gid) else { return };
            let total: usize = p.parts.iter().flatten().map(Vec::len).sum();
            let mut whole = Vec::with_capacity(total);
            for part in p.parts.into_iter().flatten() {
                whole.extend_from_slice(&part);
            }
            deliver(&whole);
        }
    }

    // ------------------------------------------------------- acks, rtt, cwnd

    fn on_ack(&mut self, now: Instant, ack: u16, bits: u32) {
        let mut sent = std::mem::take(&mut self.sent);
        let mut acked_bytes = 0usize;
        let mut lost = false;
        let mut sample = None;
        {
            let Connection {
                unacked, base_id, ..
            } = self;
            let base = *base_id;
            sent.retain(|sp| {
                let d = ack.wrapping_sub(sp.seq);
                if d >= 0x8000 {
                    return true;
                }
                let is_acked = d == 0 || (d <= 32 && bits & (1 << (d - 1)) != 0);
                if is_acked {
                    if d == 0 {
                        sample = Some(now.saturating_duration_since(sp.at));
                    }
                    for id in &sp.ids {
                        let Some(r) = id
                            .checked_sub(base)
                            .and_then(|i| unacked.get_mut(i as usize))
                        else {
                            continue;
                        };
                        if !r.acked {
                            r.acked = true;
                            r.data = Vec::new();
                            if r.sent_at.is_some() {
                                acked_bytes += r.len;
                            }
                        }
                    }
                    false
                } else if d >= 3 {
                    for id in &sp.ids {
                        let Some(r) = id
                            .checked_sub(base)
                            .and_then(|i| unacked.get_mut(i as usize))
                        else {
                            continue;
                        };
                        if !r.acked && r.sent_at == Some(sp.at) {
                            r.resend_now = true;
                            lost = true;
                        }
                    }
                    false
                } else {
                    true
                }
            });
        }
        self.sent = sent;

        while self.unacked.front().is_some_and(|r| r.acked) {
            self.unacked.pop_front();
            self.base_id += 1;
            self.scan = self.scan.saturating_sub(1);
        }
        self.in_flight = self.in_flight.saturating_sub(acked_bytes);

        if let Some(s) = sample {
            self.rtt_sample(s);
        }
        if lost {
            self.on_loss(now);
        }
        if acked_bytes > 0 {
            let mtu = self.p.mtu;
            if self.cwnd < self.ssthresh {
                self.cwnd += acked_bytes;
            } else {
                self.cwnd += (mtu * acked_bytes / self.cwnd).max(1);
            }
            self.cwnd = self.cwnd.min(WINDOW * mtu);
        }
    }
    fn on_loss(&mut self, now: Instant) {
        if now < self.recovery_until {
            return;
        }
        self.ssthresh = (self.cwnd / 2).max(2 * self.p.mtu);
        self.cwnd = self.ssthresh;
        self.recovery_until = now + self.srtt.unwrap_or(self.rto);
    }

    fn rtt_sample(&mut self, r: Duration) {
        match self.srtt {
            None => {
                self.srtt = Some(r);
                self.rttvar = r / 2;
            }
            Some(s) => {
                let diff = if s > r { s - r } else { r - s };
                self.rttvar = self.rttvar * 3 / 4 + diff / 4;
                self.srtt = Some(s * 7 / 8 + r / 8);
            }
        }
        let s = self.srtt.unwrap_or(r);
        self.rto = (s + (self.rttvar * 4).max(Duration::from_millis(10))).clamp(MIN_RTO, MAX_RTO);
    }
}
