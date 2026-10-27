use raylib::{RaylibHandle, drawing::RaylibTextureMode, math::Vector2};
use wrum_core::sim::Status;

use crate::{
    assets::Assets,
    camera::Camera,
    ctx::Context,
    stack::{Part, Stack},
};

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
    pub status: Status,
    pub invuln: u8,
    pub sunk_at: f32,
    pub drift_dir: i8,
    pub tier: u8,
    pub charge: u16,
    pub boost: u8,
    pub boost_lvl: u8,
    tire: Stack,
    body: Stack,
}

impl Car {
    pub fn new(ctx: &mut Context, p: Vector2, a: f32, t: f32, z: f32) -> Car {
        Car {
            p,
            z,
            a,
            t,
            status: Status::Normal,
            invuln: 0,
            sunk_at: 0.,
            tire: Stack::new(ctx, "assets/tire.png", 1.),
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
        let parts = self.parts(a);

        if self.z >= 0.0 {
            for (st, p, ang) in &parts {
                st.draw_part(assets, tm, *p, *ang, camera, self.z, Part::All, alpha);
            }
            return;
        }
        for (st, p, ang) in &parts {
            st.draw_part(assets, tm, *p, *ang, camera, self.z, Part::Submerged, 255);
        }
        overlay(tm, self.p, -self.z);
        for (st, p, ang) in &parts {
            st.draw_part(assets, tm, *p, *ang, camera, self.z, Part::Dry, alpha);
        }
    }
    pub fn draw_shadow(&self, tm: &mut RaylibTextureMode<'_, '_, RaylibHandle>, camera: &Camera) {
        let lift = (self.z / 30.0).clamp(0.0, 1.0);
        let fade = (1.0 + self.z / 6.0).clamp(0.0, 1.0);
        let alpha = (255.0 * (1.0 - 0.55 * lift) * fade) as u8;
        if alpha == 0 {
            return;
        }
        let off = SHADOW_OFFSET * (1.0 + self.z.max(0.0) * 0.05);
        for (st, p, ang) in self.parts(self.a) {
            st.draw_shadow(tm, p, ang, camera, off, alpha);
        }
    }

    fn parts(&self, a_deg: f32) -> [(&Stack, Vector2, f32); TIRE_COUNT + 1] {
        let rad = a_deg.to_radians();
        let (s, c) = rad.sin_cos();
        let tire = |i: usize| {
            let b = TIRE_POSITIONS[i];
            let w = Vector2 {
                x: self.p.x + b.x * c - b.y * s,
                y: self.p.y + b.x * s + b.y * c,
            };
            (&self.tire, w, a_deg + if i < 2 { self.t } else { 0.0 })
        };
        [
            tire(0),
            tire(1),
            tire(2),
            tire(3),
            (&self.body, self.p, a_deg),
        ]
    }
}
