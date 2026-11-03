use std::time::Instant;

use wrum_core::{
    sim::grid::Map,
    sim::{DT, Input, InputBuf, MAX_CARS, V2, World},
};
use wrum_proto::ClientMsg;

const ERR_DECAY_ME: f32 = 12.0;
const ERR_DECAY_REMOTE: f32 = 7.0;

pub struct NetSim {
    pub me: usize,
    map: Map,
    t0: Instant,

    cur: u32,
    acc: f32,
    rate: f32,
    playing: bool,

    pings: u32,
    pongs: u32,
    last_ping: f64,
    last_snap: u32,
    got_snap: bool,
    sync_start: f64,
    best_rtt: f32,
    best_srv: f64,
    best_at: f64,
    freeze_until: f64,
    win: [i8; 40],
    win_n: usize,
    win_i: usize,
    pub rtt: f32,
    pub early: i32,
    pub max_err: f32,
    last_log: f64,

    pub w: World,
    prev: World,
    have_world: bool,
    pending: Option<World>,
    pub go_tick: Option<u32>,
    my_in: InputBuf,
    rem_in: [InputBuf; MAX_CARS],
    err_pos: [V2; MAX_CARS],
    err_rot: [f32; MAX_CARS],
    alpha: f32,
    out: Vec<Input>,

    margin: f32,
    last_late: f64,
    last_adj: f64,
}

fn wrap_pi(a: f32) -> f32 {
    use std::f32::consts::{PI, TAU};
    (a + PI).rem_euclid(TAU) - PI
}

impl NetSim {
    pub fn new(map: Map, me: usize) -> Self {
        Self {
            me,
            map,
            t0: Instant::now(),
            cur: 0,
            acc: 0.0,
            rate: 1.0,
            playing: false,
            pings: 0,
            pongs: 0,
            last_ping: -1.0,
            last_snap: 0,
            got_snap: false,
            sync_start: 0.0,
            best_rtt: 0.0,
            best_srv: 0.0,
            best_at: 0.0,
            freeze_until: 0.0,
            win: [0; 40],
            win_n: 0,
            win_i: 0,
            rtt: 0.0,
            early: 0,
            max_err: 0.0,
            w: World::default(),
            prev: World::default(),
            have_world: false,
            pending: None,
            go_tick: None,
            my_in: InputBuf::default(),
            rem_in: std::array::from_fn(|_| InputBuf::default()),
            err_pos: [V2::ZERO; MAX_CARS],
            err_rot: [0.0; MAX_CARS],
            alpha: 0.0,
            out: Vec::new(),
            margin: 3.0,
            last_late: 0.0,
            last_adj: 0.0,
            last_log: 0.0,
        }
    }

    pub fn have_world(&self) -> bool {
        self.have_world
    }
    pub fn tick(&self) -> u32 {
        self.cur
    }
    #[cfg(feature = "debug-renderer")]
    pub fn rate(&self) -> f32 {
        self.rate
    }
    #[cfg(feature = "debug-renderer")]
    pub fn lead_ticks(&self) -> i32 {
        self.cur.wrapping_sub(self.w.tick.min(self.cur)) as i32
    }
    #[cfg(feature = "debug-renderer")]
    pub fn syncing(&self) -> bool {
        !self.playing
    }
    #[cfg(feature = "debug-renderer")]
    pub fn margin(&self) -> f32 {
        self.margin
    }

    fn now(&self) -> f64 {
        self.t0.elapsed().as_secs_f64()
    }
    fn now_us(&self) -> u32 {
        self.t0.elapsed().as_micros() as u32
    }

    // ------------------------------------------------------------ update

    pub fn update(&mut self, dt: f32, input: Input, tx: &mut Vec<ClientMsg>) {
        let dt = dt.min(1.0);
        if dt > 0.05 {
            eprintln!("FRAME stall {:.0}ms", dt * 1000.0);
        }

        let now = self.now();
        if !self.playing {
            if self.pings < 8 && now - self.last_ping > 0.05 {
                self.ping(now, tx);
            }
            if self.pongs >= 6 || (self.pongs >= 3 && now - self.sync_start > 1.5) {
                self.start_playing(now);
            }
            return;
        }
        if now - self.last_ping > 1.0 {
            self.ping(now, tx);
        }
        if let Some(s) = self.pending.take() {
            self.apply_snapshot(s);
        }

        self.acc = (self.acc + dt * self.rate).min(40.0 * DT);
        let first = self.cur;
        self.out.clear();
        while self.acc >= DT {
            self.acc -= DT;
            self.tick_once(input);
        }
        self.alpha = (self.acc / DT).clamp(0.0, 1.0);
        if !self.out.is_empty() {
            let skip = self.out.len().saturating_sub(8);
            tx.push(ClientMsg::Inputs {
                first: first.wrapping_add(skip as u32),
                inputs: self.out[skip..].to_vec(),
            });
        }

        let k_me = (-ERR_DECAY_ME * dt).exp();
        let k_rem = (-ERR_DECAY_REMOTE * dt).exp();
        for i in 0..MAX_CARS {
            let k = if i == self.me { k_me } else { k_rem };
            self.err_pos[i] *= k;
            self.err_rot[i] *= k;
        }
    }

    fn tick_once(&mut self, input: Input) {
        self.my_in.push(self.cur, input);
        self.out.push(input);
        if self.have_world {
            self.prev = self.w;
            self.step_once();
        }
        self.cur = self.cur.wrapping_add(1);
    }
    fn step_once(&mut self) {
        let t = self.w.tick;
        let racing = self
            .go_tick
            .is_some_and(|g| (t.wrapping_sub(g) as i32) >= 0);
        let mut ins = [Input::default(); MAX_CARS];
        if racing {
            for i in 0..MAX_CARS {
                ins[i] = if i == self.me {
                    self.my_in.at(t)
                } else {
                    self.rem_in[i].at(t)
                };
            }
        }
        self.w.step(&self.map, &ins, true, &mut |_, _| {});
    }

    // ------------------------------------------------------------ clock

    fn ping(&mut self, now: f64, tx: &mut Vec<ClientMsg>) {
        self.pings += 1;
        self.last_ping = now;
        tx.push(ClientMsg::Ping { t0: self.now_us() });
    }
    pub fn on_pong(&mut self, t0: u32, tick: u32, frac: f32) {
        let now = self.now();
        let rtt = self.now_us().wrapping_sub(t0) as f32 / 1e6;
        self.pongs += 1;
        self.rtt = if self.rtt == 0.0 {
            rtt
        } else {
            self.rtt + (rtt - self.rtt) * 0.1
        };
        if !self.playing && (self.best_rtt == 0.0 || rtt < self.best_rtt) {
            self.best_rtt = rtt;
            self.best_srv = tick as f64 + frac as f64 + (rtt / 2.0 / DT) as f64;
            self.best_at = now;
        }
    }
    fn start_playing(&mut self, now: f64) {
        let srv = self.best_srv + (now - self.best_at) / DT as f64;
        self.cur = (srv + (self.best_rtt / 2.0 / DT) as f64 + self.margin as f64 + 1.0) as u32;
        self.acc = 0.0;
        self.rate = 1.0;
        self.my_in.reset(self.cur);
        self.playing = true;
    }
    fn on_early(&mut self, early: i8) {
        let now = self.now();
        if !self.playing || early == 127 || now < self.freeze_until {
            return;
        }
        if early < 0 {
            if now - self.last_log > 1.0 {
                eprintln!(
                    "LATE early={early} margin={} rtt {:.0}ms",
                    self.margin,
                    self.rtt * 1000.0
                );
                self.last_log = now;
            }
            if early >= -8 && now - self.last_adj > 0.5 {
                self.margin = (self.margin + 1.0).min(8.0);
                self.last_adj = now;
                self.win_n = 0;
                self.win_i = 0;
            }
            self.last_late = now;
        } else if now - self.last_late > 15.0 && now - self.last_adj > 15.0 && self.margin > 3.0 {
            self.margin -= 1.0;
            self.last_adj = now;
        }

        self.win[self.win_i % 20] = early;
        self.win_i += 1;
        self.win_n = (self.win_n + 1).min(20);
        let m = self.win[..self.win_n].iter().copied().min().unwrap() as f32;
        self.early = m as i32;
        let e = m - self.margin;
        if e < -12.0 || e > 20.0 {
            self.acc += -e * DT;
            self.win_n = 0;
            self.win_i = 0;
            self.freeze_until = now + (self.rtt * 1.5 + 0.1) as f64;
            self.rate = 1.0;
        } else if e < 0.0 {
            self.rate = 1.0 + (-e * 0.03).min(0.10);
        } else if e > 2.0 {
            self.rate = 1.0 - ((e - 2.0) * 0.01 + 0.01).min(0.05);
        } else {
            self.rate = 1.0;
        }
    }

    // ------------------------------------------------------------ net

    pub fn on_relay(&mut self, slot: usize, first: u32, inputs: &[Input]) {
        if slot >= MAX_CARS || slot == self.me {
            return;
        }
        for (k, i) in inputs.iter().enumerate() {
            self.rem_in[slot].push(first.wrapping_add(k as u32), *i);
        }
    }
    pub fn on_player_changed(&mut self, slot: usize) {
        if slot < MAX_CARS {
            self.rem_in[slot] = InputBuf::default();
        }
    }
    pub fn on_snapshot(&mut self, s: &World, early: i8) {
        if self.got_snap && (s.tick.wrapping_sub(self.last_snap) as i32) <= 0 {
            return;
        }
        self.got_snap = true;
        self.last_snap = s.tick;
        self.on_early(early);
        self.pending = Some(*s);
    }

    fn apply_snapshot(&mut self, s: World) {
        let before: [Option<(V2, f32)>; MAX_CARS] = std::array::from_fn(|i| {
            (self.have_world && self.w.cars[i].active).then(|| {
                let v = self.view(i);
                (v.0, v.1)
            })
        });

        self.w = s;
        self.prev = s;
        self.have_world = true;
        self.err_pos = [V2::ZERO; MAX_CARS];
        self.err_rot = [0.0; MAX_CARS];

        if (s.tick.wrapping_sub(self.cur) as i32) > 0 {
            self.cur = s.tick;
            return;
        }
        while self.w.tick != self.cur {
            if self.w.tick.wrapping_add(1) == self.cur {
                self.prev = self.w;
            }
            self.step_once();
        }

        self.max_err = 0.0;
        for i in 0..MAX_CARS {
            let Some((bp, br)) = before[i] else { continue };
            if !self.w.cars[i].active {
                continue;
            }
            let (np, nr, _) = self.view(i);
            let (d, dr) = (bp - np, wrap_pi(br - nr));
            if d.length() > 64.0 || dr.abs() > 1.5 {
                continue;
            }
            self.err_pos[i] = d;
            self.err_rot[i] = dr;
            self.max_err = self.max_err.max(d.length());
        }
        let own = self.err_pos[self.me].length();
        let rem = (0..MAX_CARS)
            .filter(|&i| i != self.me)
            .map(|i| self.err_pos[i].length())
            .fold(0.0f32, f32::max);
        if own > 2.0 || rem > 4.0 {
            eprintln!(
                "CORR own {:.1}px remote {:.1}px | replay {} | early {} | rtt {:.0}ms | rate {:.3}",
                own,
                rem,
                self.cur.wrapping_sub(s.tick),
                self.early,
                self.rtt * 1000.0,
                self.rate,
            );
        }
    }

    // ------------------------------------------------------------ render

    pub fn view(&self, i: usize) -> (V2, f32, f32) {
        let (a, b) = (&self.prev.cars[i], &self.w.cars[i]);
        let (mut p, mut r, mut z) = (b.pos, b.rot, b.z);
        if a.active && (b.pos - a.pos).length() < 40.0 {
            let al = self.alpha;
            p = a.pos.lerp(b.pos, al);
            r = a.rot + wrap_pi(b.rot - a.rot) * al;
            z = a.z + (b.z - a.z) * al;
        }
        (p + self.err_pos[i], r + self.err_rot[i], z)
    }
    #[cfg(feature = "debug-renderer")]
    pub fn map(&self) -> &wrum_core::sim::grid::Map {
        &self.map
    }
}
