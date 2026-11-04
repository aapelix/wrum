use raylib::{RaylibHandle, drawing::RaylibTextureMode, math::Vector2};
use wrum_core::sim::Status;

use crate::{
    assets::Assets,
    camera::Camera,
    ctx::Context,
    stack::{Part, Stack},
};

pub const SHADOW_OFFSET: Vector2 = Vector2 { x: 1., y: 0.5 };
pub const TIRE_POSITIONS: [Vector2; 4] = [
    Vector2 { x: -3.5, y: -4.0 },
    Vector2 { x: 3.5, y: -4.0 },
    Vector2 { x: -3.5, y: 4.0 },
    Vector2 { x: 3.5, y: 4.0 },
];

#[derive(Clone)]
pub struct Car {
    pub p: Vector2,
    pub z: f32,
    pub a: f32,
    pub status: Status,
    pub invuln: u8,
    pub sunk_at: f32,
    pub drift_dir: i8,
    pub tier: u8,
    pub charge: u16,
    pub boost: u8,
    pub boost_lvl: u8,
    body: Stack,
}

impl Car {
    pub fn new(ctx: &mut Context, p: Vector2, a: f32, z: f32) -> Car {
        Car {
            p,
            z,
            a,
            status: Status::Normal,
            invuln: 0,
            sunk_at: 0.,
            body: Stack::new(ctx, "assets/body.png", 10.),
            drift_dir: 0,
            tier: 0,
            charge: 0,
            boost: 0,
            boost_lvl: 0,
        }
    }
    pub fn draw(
        &self,
        assets: &Assets,
        tm: &mut RaylibTextureMode<'_, '_, RaylibHandle>,
        camera: &Camera,
        now: f32,
        overlay: impl FnOnce(&mut RaylibTextureMode<'_, '_, RaylibHandle>, Vector2, f32),
        ghost: bool,
    ) {
        let mut a = self.a;
        if self.status == Status::Drowning {
            let t = now - self.sunk_at;
            a += (t * 7.0).sin() * 6.0 * (-t * 2.0).exp();
        }
        let alpha = if self.invuln > 0 && (now * 4.0) as i32 % 2 == 0 {
            160
        } else {
            255
        };
        let alpha = if ghost { 160 } else { alpha };

        if self.z >= 0.0 {
            self.body
                .draw_part(assets, tm, self.p, a, camera, self.z, Part::All, alpha);
            return;
        }

        self.body
            .draw_part(assets, tm, self.p, a, camera, self.z, Part::Submerged, 255);
        overlay(tm, self.p, -self.z);
        self.body
            .draw_part(assets, tm, self.p, a, camera, self.z, Part::Dry, alpha);
    }
    pub fn draw_shadow(
        &self,
        assets: &Assets,
        tm: &mut RaylibTextureMode<'_, '_, RaylibHandle>,
        camera: &Camera,
    ) {
        let lift = (self.z / 30.0).clamp(0.0, 1.0);
        let fade = (1.0 + self.z / 6.0).clamp(0.0, 1.0);
        let alpha = (255.0 * (1.0 - 0.55 * lift) * fade) as u8;
        if alpha == 0 {
            return;
        }
        let off = SHADOW_OFFSET * (1.0 + self.z.max(0.0) * 0.05);
        self.body
            .draw_shadow(assets, tm, self.p, self.a, camera, off, alpha);
    }
}
