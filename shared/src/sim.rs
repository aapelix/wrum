use std::f32::consts::PI;

use rkyv::{Archive, Deserialize, Serialize};

use crate::coll::{CollisionBody, Obb, Vec2, resolve_collision};

const MAX_V: f32 = 200.0;
const ACC: f32 = 5.0;
const DRAG: f32 = 5.0;
const RS: f32 = 0.1;
const MAX_A: f32 = 30.0 * PI / 180.0;
const TIRE_RS: f32 = 0.05;
const TIRE_TS: f32 = 0.1;

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

    pub fn step(&mut self, input: &Input) {
        let steer = input.steer.clamp(-1, 1) as f32;
        let thr = input.thr.clamp(-1, 1) as f32;

        self.s = approach(self.s, thr * MAX_V, if thr != 0.0 { ACC } else { DRAG });

        self.t = approach(
            self.t,
            steer * MAX_A,
            if steer != 0.0 { TIRE_TS } else { TIRE_RS },
        );

        self.a += self.t * (self.s / MAX_V) * RS;

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
}
