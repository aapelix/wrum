use glam::Vec2;
use rkyv::{Archive, Deserialize, Serialize};

pub mod grid;

pub use glam::Vec2 as V2;

use crate::sim::grid::{Map, crossed};

pub const DT: f32 = 1.0 / 60.0;
pub const MAX_CARS: usize = 8;

pub const HW: f32 = 4.0;
pub const HL: f32 = 7.5;
pub const RADIUS: f32 = HW;
pub const OFFSET: f32 = HL - HW;

const GRAVITY: f32 = 400.0;
const PAD_MIN_SPEED: f32 = 150.0;
const CLEARANCE: f32 = 6.0;
const HIT_MIN_SPEED: f32 = 70.0;
const HIT_HOP: f32 = 60.0;
const STUN_TICKS: u16 = 45;
const INVULN_TICKS: u8 = 60;
const SPIN_RATE: f32 = 18.0;
const WATER_DRAG: f32 = 4.0;
const SINK_ACCEL: f32 = 80.0;
const SINK_DRAG: f32 = 2.5;
pub const MAX_DEPTH: f32 = 24.0;
const DROWN_TICKS: u16 = 100;

pub const MAX_SPEED: f32 = 180.0;
const MAX_REVERSE: f32 = 120.0;
const ACCEL: f32 = 220.0;
const BRAKE: f32 = 450.0;
const COAST_DECEL: f32 = 120.0;
const LATERAL_GRIP: f32 = 16.0;
const TURN_RATE: f32 = 2.8;
const TURN_SMOOTH: f32 = 14.0;
const MIN_TURN_SPEED: f32 = 25.0;

const WALL_BOUNCE: f32 = 0.2;
const CAR_BOUNCE: f32 = 0.1;
const MAX_PUSH: f32 = 2.0;

const DRIFT_GRIP: f32 = 5.0;
const DRIFT_TURN: f32 = TURN_RATE * 1.15;
const DRIFT_MIN_SPEED: f32 = 85.0;
const DRIFT_MIN_ANG: f32 = 0.9;
pub const CHARGE_TIERS: [u16; 3] = [60, 135, 225];
const BOOST_TICKS: [u8; 3] = [40, 65, 95];
const BOOST_MULT: [f32; 3] = [1.22, 1.32, 1.42];
const BOOST_ACCEL: f32 = 700.0;
const BOOST_KICK: f32 = 25.0;
const ABS_MAX: f32 = MAX_SPEED * 1.5;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum Surface {
    Road = 0,
    Grass = 1,
    Water = 2,
}
struct SurfParams {
    max_speed: f32,
    accel: f32,
    grip: f32,
    extra_decel: f32,
}
impl Surface {
    pub fn from_u8(v: u8) -> Self {
        match v & 3 {
            1 => Surface::Grass,
            2 => Surface::Water,
            _ => Surface::Road,
        }
    }
    fn params(self) -> SurfParams {
        match self {
            Surface::Grass => SurfParams {
                max_speed: 0.45,
                accel: 0.6,
                grip: 0.7,
                extra_decel: 200.0,
            },
            _ => SurfParams {
                max_speed: 1.0,
                accel: 1.0,
                grip: 1.0,
                extra_decel: 0.0,
            },
        }
    }
}

#[derive(Archive, Serialize, Deserialize, Debug, Clone, Copy, Default, PartialEq)]
pub struct Input {
    pub throttle: i8,
    pub steer: i8,
    pub drift: bool,
}
impl Input {
    fn clamped(self) -> Self {
        Self {
            throttle: self.throttle.clamp(-1, 1),
            steer: self.steer.clamp(-1, 1),
            drift: self.drift,
        }
    }
}

#[derive(Archive, Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Status {
    #[default]
    Normal,
    Spun,
    Drowning,
}

#[derive(Archive, Serialize, Deserialize, Clone, Copy, Debug, Default)]
pub struct Car {
    pub active: bool,
    pub pos: Vec2,
    pub vel: Vec2,
    pub rot: f32,
    pub ang: f32,
    pub z: f32,
    pub vz: f32,
    pub status: Status,
    pub timer: u16,
    pub invuln: u8,
    pub age: u32,
    pub safe: Vec2,
    pub safe_rot: f32,
    pub safe_next: Vec2,
    pub safe_next_rot: f32,

    pub drift_dir: i8,
    pub charge: u16,
    pub boost: u8,
    pub boost_lvl: u8,
}
impl Car {
    pub fn speed(&self) -> f32 {
        self.vel.length()
    }
    fn solid(&self) -> bool {
        self.status != Status::Drowning && self.z <= CLEARANCE
    }
    fn can_hit(&self) -> bool {
        self.status == Status::Normal && self.invuln == 0 && self.z < CLEARANCE
    }
    pub fn circles(&self) -> [Vec2; 2] {
        let (f, _) = axes(self.rot);
        [self.pos + f * OFFSET, self.pos - f * OFFSET]
    }
    pub fn tier(&self) -> u8 {
        CHARGE_TIERS.iter().filter(|&&t| self.charge >= t).count() as u8
    }
}

#[derive(Archive, Serialize, Deserialize, Clone, Copy, Debug)]
pub struct World {
    pub cars: [Car; MAX_CARS],
    pub tick: u32,
}
impl Default for World {
    fn default() -> Self {
        Self {
            cars: [Car::default(); MAX_CARS],
            tick: 0,
        }
    }
}

fn axes(rot: f32) -> (Vec2, Vec2) {
    let (s, c) = libm::sincosf(rot);
    (Vec2::new(s, -c), Vec2::new(c, s))
}
fn decay(k: f32) -> f32 {
    1.0 / (1.0 + k * DT)
}
fn wrap(a: f32) -> f32 {
    use std::f32::consts::{PI, TAU};
    if a > PI {
        a - TAU
    } else if a < -PI {
        a + TAU
    } else {
        a
    }
}

impl World {
    pub fn spawn(&mut self, slot: usize, pos: Vec2, rot: f32) {
        self.cars[slot] = Car {
            active: true,
            pos,
            rot,
            safe: pos,
            safe_rot: rot,
            safe_next: pos,
            safe_next_rot: rot,
            ..Default::default()
        };
    }
    pub fn remove(&mut self, slot: usize) {
        self.cars[slot] = Car::default();
    }

    pub fn step(
        &mut self,
        map: &Map,
        inputs: &[Input; MAX_CARS],
        hits: bool,
        on_gate: &mut dyn FnMut(usize, u32),
    ) {
        let mut prev = [Vec2::ZERO; MAX_CARS];
        for i in 0..MAX_CARS {
            let c = &mut self.cars[i];
            if !c.active {
                continue;
            }
            drive(c, inputs[i].clamped(), map);
            prev[i] = c.pos;
            c.pos += c.vel * DT;
            c.rot = wrap(c.rot + c.ang * DT);
            if c.solid() {
                collide_walls(c, map);
            }
        }
        for i in 0..MAX_CARS {
            for j in i + 1..MAX_CARS {
                if self.cars[i].active && self.cars[j].active {
                    let (l, r) = self.cars.split_at_mut(j);
                    collide_pair(&mut l[i], &mut r[0], hits);
                }
            }
        }
        for i in 0..MAX_CARS {
            let c = &self.cars[i];
            if !c.active || c.status == Status::Drowning {
                continue;
            }
            for g in &map.gates {
                if crossed(g, prev[i], c.pos) {
                    on_gate(i, g.index);
                }
            }
        }
        self.tick = self.tick.wrapping_add(1);
    }
}

fn drive(c: &mut Car, inp: Input, map: &Map) {
    let surf = map.surface_at(c.pos);
    let sp = surf.params();
    c.invuln = c.invuln.saturating_sub(1);
    c.age = c.age.wrapping_add(1);

    let mut landed = 0.0;
    if c.status != Status::Drowning && (c.z > 0.0 || c.vz > 0.0) {
        c.vz -= GRAVITY * DT;
        c.z += c.vz * DT;
        if c.z <= 0.0 {
            landed = c.vz;
            c.z = 0.0;
            c.vz = 0.0;
        }
    }
    let grounded = c.z <= 0.0;

    if grounded && surf == Surface::Water && c.status != Status::Drowning {
        c.status = Status::Drowning;
        c.timer = DROWN_TICKS;
        c.vz = landed * 0.35 - 25.0;

        c.drift_dir = 0;
        c.charge = 0;
        c.boost = 0;
        c.boost_lvl = 0;
    }
    if c.status == Status::Drowning {
        c.vz = (c.vz - SINK_ACCEL * DT) * decay(SINK_DRAG);
        c.z = (c.z + c.vz * DT).max(-MAX_DEPTH);
        c.vel *= decay(WATER_DRAG);
        c.ang *= decay(3.0);
        c.timer = c.timer.saturating_sub(1);
        if c.timer == 0 {
            let (f, _) = axes(c.safe_rot);
            c.pos = c.safe;
            c.rot = c.safe_rot;
            c.vel = f * 40.0;
            c.ang = 0.0;
            c.z = 0.0;
            c.vz = 0.0;
            c.status = Status::Normal;
            c.invuln = 90;
            c.safe_next = c.safe;
            c.safe_next_rot = c.safe_rot;
        }
        return;
    }

    // jump pad
    let pad = if grounded && c.status == Status::Normal {
        map.pad_at(c.pos)
    } else {
        None
    };
    if let Some((power, dir)) = pad {
        c.vz = power;
        c.z = 0.01;
        let speed = c.vel.dot(dir).max(PAD_MIN_SPEED);
        c.vel = dir * speed;
    }
    // speed pad
    if grounded && c.status == Status::Normal {
        if let Some(level) = map.speed_pad_at(c.pos) {
            let level = level as usize - 1;

            c.boost = c.boost.max(BOOST_TICKS[level]);
            c.boost_lvl = c.boost_lvl.max(level as u8 + 1);

            let (f, _) = axes(c.rot);
            c.vel += f * (BOOST_KICK + 20.0).max(0.0);

            let forward = c.vel.dot(f);
            if forward < MAX_SPEED * BOOST_MULT[level] {
                c.vel += f * (MAX_SPEED * BOOST_MULT[level] - forward);
            }
        }
    }

    let air = c.z > 0.0;
    let stunned = c.status == Status::Spun;

    let (f, r) = axes(c.rot);
    let mut fwd = c.vel.dot(f);
    let mut lat = c.vel.dot(r);
    let speed = c.vel.length();
    let (grip, turn) = if inp.drift {
        (DRIFT_GRIP, DRIFT_TURN)
    } else {
        (LATERAL_GRIP, TURN_RATE)
    };

    if c.drift_dir == 0 {
        if inp.drift && grounded && !stunned && fwd > DRIFT_MIN_SPEED && inp.steer != 0 {
            c.drift_dir = inp.steer;
            c.charge = 0;
        }
    } else {
        let keep = inp.drift && !stunned;

        if !keep {
            let t = c.tier();

            let valid_release = !inp.drift
                && speed > DRIFT_MIN_SPEED * 0.6
                && grounded
                && !stunned
                && surf == Surface::Road;

            if valid_release && t > 0 {
                c.boost = BOOST_TICKS[t as usize - 1];
                c.boost_lvl = t;
                fwd = fwd.max(MAX_SPEED) + BOOST_KICK;
                lat *= 0.3;
            } else {
                fwd *= 0.9;
            }

            c.drift_dir = 0;
            c.charge = 0;
        } else if grounded && c.ang * c.drift_dir as f32 > DRIFT_MIN_ANG {
            let inc = match inp.steer * c.drift_dir {
                1 => 3,
                0 => 2,
                _ => 1,
            };

            c.charge = (c.charge + inc).min(CHARGE_TIERS[2] + 30);
        }
    }
    let drifting = c.drift_dir != 0;

    if air {
        // momentum only
    } else if stunned {
        fwd *= decay(2.0);
    } else {
        let max_f = if c.boost > 0 {
            MAX_SPEED * BOOST_MULT[c.boost_lvl as usize - 1]
        } else {
            MAX_SPEED * sp.max_speed
        };

        let max_r = MAX_REVERSE * sp.max_speed;
        let th = if c.boost > 0 {
            1.0
        } else {
            inp.throttle as f32
        };

        if th != 0.0 {
            let braking = fwd != 0.0 && th.signum() != fwd.signum();
            let rate = if c.boost > 0 {
                BOOST_ACCEL
            } else if braking {
                BRAKE
            } else {
                ACCEL * sp.accel
            };
            fwd += th * rate * DT;
        } else {
            let d = COAST_DECEL * DT;
            fwd = if fwd.abs() <= d {
                0.0
            } else {
                fwd - fwd.signum() * d
            };
        }

        let over = (COAST_DECEL + sp.extra_decel) * DT;
        if fwd > max_f {
            fwd = (fwd - over).max(max_f);
        } else if fwd < -max_r {
            fwd = (fwd + over).min(-max_r);
        }
    }

    if !air {
        let max_speed = if c.boost > 0 { ABS_MAX } else { MAX_SPEED };
        fwd = fwd.clamp(-MAX_REVERSE, max_speed);
    }

    c.boost = c.boost.saturating_sub(1);
    if c.boost == 0 {
        c.boost_lvl = 0;
    }

    if !air {
        let g = if stunned { DRIFT_GRIP * 0.5 } else { grip };
        lat *= decay(g * sp.grip);
    }
    c.vel = f * fwd + r * lat;

    let steer = if drifting {
        c.drift_dir as f32 + 0.5 * inp.steer as f32
    } else {
        inp.steer as f32
    };
    let target = if stunned {
        let dir = if c.ang >= 0.0 { 1.0 } else { -1.0 };
        dir * SPIN_RATE * (c.timer as f32 / STUN_TICKS as f32).max(0.25)
    } else {
        let ctl = if air { 0.35 } else { 1.0 };
        steer * turn * (fwd / MIN_TURN_SPEED).clamp(-1.0, 1.0) * ctl
    };
    c.ang += (target - c.ang) * (1.0 - decay(TURN_SMOOTH));

    if stunned {
        c.timer = c.timer.saturating_sub(1);
        if c.timer == 0 {
            c.status = Status::Normal;
        }
    }
    if !air && !stunned && surf == Surface::Road && c.age % 30 == 0 {
        c.safe = c.safe_next;
        c.safe_rot = c.safe_next_rot;
        c.safe_next = c.pos;
        c.safe_next_rot = c.rot;
    }
}

fn collide_walls(c: &mut Car, map: &Map) {
    let (f, _) = axes(c.rot);
    for sgn in [1.0f32, -1.0] {
        let mut p = c.pos + f * (sgn * OFFSET);
        for &si in map.nearby(p) {
            let d = p - map.segs[si as usize].closest(p);
            let d2 = d.length_squared();
            if d2 >= RADIUS * RADIUS || d2 == 0.0 {
                continue;
            }
            let dist = d2.sqrt();
            let n = d / dist;
            let push = n * (RADIUS - dist);
            c.pos += push;
            p += push;
            let vn = c.vel.dot(n);
            if vn < 0.0 {
                c.vel -= n * (vn * (1.0 + WALL_BOUNCE));
            }
        }
    }
}
fn collide_pair(a: &mut Car, b: &mut Car, hits: bool) {
    if !a.solid() || !b.solid() {
        return;
    }
    let d = b.pos - a.pos;
    if d.length_squared() > 4.0 * HL * HL {
        return;
    }
    let (va, vb) = (a.vel, b.vel);
    let (fa, _) = axes(a.rot);
    let (fb, _) = axes(b.rot);
    let mut touched = false;
    for sa in [1.0f32, -1.0] {
        for sb in [1.0f32, -1.0] {
            let pa = a.pos + fa * (sa * OFFSET);
            let pb = b.pos + fb * (sb * OFFSET);
            let dd = pb - pa;
            let d2 = dd.length_squared();
            if d2 >= 4.0 * RADIUS * RADIUS || d2 == 0.0 {
                continue;
            }
            touched = true;
            let dist = d2.sqrt();
            let n = dd / dist;
            let pen = (2.0 * RADIUS - dist).min(MAX_PUSH);
            a.pos -= n * (pen * 0.5);
            b.pos += n * (pen * 0.5);
            let rel = (a.vel - b.vel).dot(n);
            if rel > 0.0 {
                let imp = n * (rel * (1.0 + CAR_BOUNCE) * 0.5);
                a.vel -= imp;
                b.vel += imp;
            }
        }
    }
    if !touched || !hits || !a.can_hit() || !b.can_hit() {
        return;
    }
    let n = (b.pos - a.pos).normalize_or_zero();
    let closing = (va - vb).dot(n);
    if closing < HIT_MIN_SPEED {
        return;
    }
    let strength = (closing / MAX_SPEED).clamp(0.3, 1.0);
    let (sa, sb) = (va.length(), vb.length());
    let head_on = (sa - sb).abs() < 25.0;
    bump(a, n, strength, sa <= sb || head_on);
    bump(b, -n, strength, sb < sa || head_on);
}
fn bump(c: &mut Car, toward: Vec2, strength: f32, victim: bool) {
    c.invuln = INVULN_TICKS;
    c.vz = HIT_HOP * strength.max(0.5);
    c.z = 0.01;
    if !victim {
        return;
    }
    let (_, r) = axes(c.rot);
    let dir = if toward.dot(r) > 0.0 { -1.0 } else { 1.0 };
    c.status = Status::Spun;
    c.timer = (STUN_TICKS as f32 * (0.5 + 0.5 * strength)) as u16;
    c.ang = dir * SPIN_RATE;
    c.vel -= toward * (40.0 * strength);
}

#[derive(Clone)]
pub struct InputBuf {
    ring: [Input; 128],
    max: u32,
    init: bool,
}
impl Default for InputBuf {
    fn default() -> Self {
        Self {
            ring: [Input::default(); 128],
            max: 0,
            init: false,
        }
    }
}
impl InputBuf {
    pub fn reset(&mut self, next: u32) {
        let last = if self.init {
            self.ring[(self.max & 127) as usize]
        } else {
            Input::default()
        };
        self.init = true;
        self.max = next.wrapping_sub(1);
        self.ring[(self.max & 127) as usize] = last;
    }
    pub fn push(&mut self, t: u32, inp: Input) -> bool {
        if !self.init {
            self.reset(t);
        }
        let d = t.wrapping_sub(self.max) as i32;
        if d <= 0 {
            return false;
        }
        if d > 100 {
            self.reset(t);
        }
        let last = self.ring[(self.max & 127) as usize];
        let mut k = self.max.wrapping_add(1);
        while k != t {
            self.ring[(k & 127) as usize] = last;
            k = k.wrapping_add(1);
        }
        self.ring[(t & 127) as usize] = inp;
        self.max = t;
        true
    }
    pub fn at(&self, n: u32) -> Input {
        if !self.init {
            Input::default()
        } else if (n.wrapping_sub(self.max) as i32) > 0 {
            self.ring[(self.max & 127) as usize]
        } else {
            self.ring[(n & 127) as usize]
        }
    }
}
