use raylib::{RaylibHandle, drawing::RaylibTextureMode, math::Vector2};
use wrum_core::physics::CarState;

use crate::{camera::Camera, ctx::Context, stack::Stack};

pub const SHADOW_OFFSET: Vector2 = Vector2 { x: 1., y: 0.5 };
pub const TIRE_COUNT: usize = 4;
pub const TIRE_POSITIONS: [Vector2; TIRE_COUNT] = [
    Vector2 { x: -3.5, y: -4.0 },
    Vector2 { x: 3.5, y: -4.0 },
    Vector2 { x: -3.5, y: 4.0 },
    Vector2 { x: 3.5, y: 4.0 },
];

pub struct Car {
    pub p: Vector2,
    pub z: f32,
    pub a: f32,
    pub t: f32,
    tire: Stack,
    body: Stack,
    pub sim: CarState,
}

impl Car {
    pub fn new(ctx: &mut Context, p: Vector2, a: f32, t: f32, z: f32) -> Car {
        Car {
            p,
            z,
            a,
            t,
            tire: Stack::new(ctx, "assets/tire.png", 1.),
            body: Stack::new(ctx, "assets/body.png", 10.),
            sim: CarState::default(),
        }
    }
    pub fn draw(&self, tm: &mut RaylibTextureMode<'_, '_, RaylibHandle>, camera: &Camera) {
        let rad = self.a.to_radians();
        let s = rad.sin();
        let c = rad.cos();

        for i in 0..TIRE_COUNT - 1 {
            let base = TIRE_POSITIONS[i];
            let world = Vector2 {
                x: self.p.x + base.x * c - base.y * s,
                y: self.p.y + base.x * s + base.y * c,
            };
            let mut draw_a = self.a;
            if i < 2 {
                draw_a += self.t;
            }

            self.tire.draw(tm, world, draw_a, camera, self.z);
        }

        self.body.draw(tm, self.p, self.a, camera, self.z);
    }
    pub fn draw_shadow(&self, tm: &mut RaylibTextureMode<'_, '_, RaylibHandle>, camera: &Camera) {
        let rad = self.a.to_radians();
        let s = rad.sin();
        let c = rad.cos();

        for i in 0..TIRE_COUNT - 1 {
            let base = TIRE_POSITIONS[i];
            let world = Vector2 {
                x: self.p.x + base.x * c - base.y * s,
                y: self.p.y + base.x * s + base.y * c,
            };
            let mut draw_a = self.a;
            if i < 2 {
                draw_a += self.t;
            }

            self.tire
                .draw_shadow(tm, world, draw_a, camera, SHADOW_OFFSET);
        }

        self.body
            .draw_shadow(tm, self.p, self.a, camera, SHADOW_OFFSET);
    }
}
