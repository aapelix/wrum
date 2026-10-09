use std::f32::consts::PI;

use glam::Vec2;
use rkyv::{Archive, Deserialize, Serialize};

use crate::{
    coll::{CollisionBody, Obb, resolve_collision},
    level::Surface,
};

const MAX_V: f32 = 200.0;
const ACC: f32 = 5.0;
const DRAG: f32 = 5.0;
const RS: f32 = 0.1;
const MAX_A: f32 = 30.0 * PI / 180.0;
const TIRE_RS: f32 = 0.05;
const TIRE_TS: f32 = 0.1;

pub const GRAVITY: f32 = 400.;
pub const SINK_DEPTH: f32 = 10.;
pub const SINK_TICKS: u16 = 50;
pub const TICK_RATE: u8 = 60;
pub const CAR_RESTITUTION: f32 = 0.1;
pub const CAR_HALFS: Vec2 = Vec2 { x: 4., y: 7.5 };
pub const TICK_DT: f32 = 1.0 / TICK_RATE as f32;

fn approach(v: f32, target: f32, step: f32) -> f32 {
    if v < target {
        (v + step).min(target)
    } else {
        (v - step).max(target)
    }
}

#[derive(Archive, Serialize, Deserialize, Debug, Default, Clone, Copy)]
pub struct Input {
    pub seq: u32,
    pub steer: i8,
    pub thr: i8,
}

#[derive(Archive, Serialize, Deserialize, Debug, Default, Clone, Copy)]
pub struct CarState {
    pub pos: Vec2,
    pub a: f32,
    pub s: f32,
    pub t: f32,

    pub z: f32,
    pub vz: f32,
    pub sink: u16,
    pub safe: Vec2,
    pub safe_a: f32,
    pub epoch: u8,
}

impl CarState {
    pub fn resolve_vs_obstacle(&mut self, obstacle: Obb, restitution: f32) {
        let mut car = CollisionBody {
            box_: self.obb(),
            vel: self.vel(),
            inv_mass: 1.0,
        };

        let mut obstacle = CollisionBody {
            box_: obstacle,
            vel: Vec2::default(),
            inv_mass: 0.0,
        };

        resolve_collision(&mut car, &mut obstacle, restitution);

        self.pos = car.box_.pos;
        self.apply_vel(car.vel);
    }

    pub fn step(&mut self, input: &Input, mods: SurfaceMods) {
        let steer = input.steer.clamp(-1, 1) as f32;
        let thr = input.thr.clamp(-1, 1) as f32;

        let max_v = MAX_V * mods.max_speed;
        let acc = ACC * mods.accel;
        let drag = DRAG * mods.drag;

        self.s = approach(self.s, thr * max_v, if thr != 0.0 { acc } else { drag });

        self.t = approach(
            self.t,
            steer * MAX_A,
            if steer != 0.0 { TIRE_TS } else { TIRE_RS },
        );

        self.a += self.t * (self.s / max_v) * RS;

        self.pos.x += self.a.sin() * self.s * TICK_DT;
        self.pos.y -= self.a.cos() * self.s * TICK_DT;

        self.a = (self.a + PI).rem_euclid(2.0 * PI) - PI;
    }

    pub fn obb(&self) -> Obb {
        Obb {
            pos: self.pos,
            halfs: CAR_HALFS,
            a: self.a,
        }
    }
    pub fn vel(&self) -> Vec2 {
        Vec2 {
            x: self.a.sin() * self.s,
            y: -self.a.cos() * self.s,
        }
    }
    pub fn apply_vel(&mut self, v: Vec2) {
        self.s = v.x * self.a.sin() + v.y * -self.a.cos()
    }

    pub fn step_sinking(&mut self) {
        self.sink += 1;

        self.vz -= GRAVITY * TICK_DT;
        if self.z < 0. {
            self.vz *= 0.9;
        }
        self.z = (self.z + self.vz * TICK_DT).max(-SINK_DEPTH);

        self.apply_vel(self.vel() * 0.99);

        if self.sink >= SINK_TICKS {
            self.pos = self.safe;
            self.a = self.safe_a;
            self.apply_vel(Vec2 { x: 0., y: 0. });
            self.z = 0.;
            self.vz = 0.;
            self.sink = 0;
            self.epoch = self.epoch.wrapping_add(1);
        }
    }
}

pub struct SurfaceMods {
    pub accel: f32,
    pub max_speed: f32,
    pub drag: f32,
}

impl SurfaceMods {
    pub fn of(s: Surface) -> Self {
        match s {
            Surface::Road => Self {
                accel: 1.0,
                max_speed: 1.0,
                drag: 1.0,
            },
            Surface::Grass => Self {
                accel: 0.6,
                max_speed: 0.55,
                drag: 2.5,
            },
            Surface::Water => Self {
                accel: 0.0,
                max_speed: 0.0,
                drag: 1.0,
            },
        }
    }
}
