use raylib::{
    RaylibHandle,
    drawing::{RaylibDraw, RaylibTextureMode},
    ffi::{Color, Vector2},
    math::Rectangle,
};

use crate::{
    assets::{AssetId, Assets},
    ctx,
};

pub mod input;

const BOX_HEIGHT: f32 = 12.;
const BOX_LEFT: Rectangle = Rectangle {
    x: 0.,
    y: 0.,
    width: 8.,
    height: BOX_HEIGHT,
};
const BOX_RIGHT: Rectangle = Rectangle {
    x: 9.,
    y: 0.,
    width: 8.,
    height: BOX_HEIGHT,
};
const BOX_MID: Rectangle = Rectangle {
    x: 8.,
    y: 0.,
    width: 1.,
    height: BOX_HEIGHT,
};
const SHADOW_COLOR: Color = Color::new(0, 0, 0, 160);

pub struct Button {
    asset_id: AssetId,
}

impl Button {
    pub fn new(ctx: &mut ctx::Context) -> Button {
        let asset_id = ctx.assets.load(ctx.rl, ctx.thread, "assets/box.png", None);
        Button { asset_id }
    }
    pub fn inside(&self, x: f32, y: f32, w: f32, point: Vector2) -> bool {
        let rect = Rectangle {
            x,
            y,
            width: w,
            height: BOX_HEIGHT,
        };
        rect.check_collision_point_rec(point)
    }
    pub fn draw(
        &self,
        assets: &Assets,
        tm: &mut RaylibTextureMode<'_, '_, RaylibHandle>,
        x: f32,
        y: f32,
        w: f32,
        mp: Vector2,
        pressed: bool,
        text: &str,
        text_centered: bool,
        force_pressed: bool,
    ) {
        let tex = assets.get(&self.asset_id);
        let inside = self.inside(x, y, w, mp);

        tm.draw_texture_pro(
            tex,
            BOX_LEFT,
            Rectangle {
                x,
                y: y + 3.,
                width: BOX_LEFT.width,
                height: BOX_HEIGHT,
            },
            Vector2::ZERO,
            0.,
            SHADOW_COLOR,
        );
        tm.draw_texture_pro(
            tex,
            BOX_RIGHT,
            Rectangle {
                x: x + w - BOX_RIGHT.width,
                y: y + 3.,
                width: BOX_RIGHT.width,
                height: BOX_HEIGHT,
            },
            Vector2::ZERO,
            0.,
            SHADOW_COLOR,
        );
        tm.draw_texture_pro(
            tex,
            BOX_MID,
            Rectangle {
                x: x + BOX_MID.x,
                y: y + 3.,
                width: w - BOX_LEFT.width - BOX_RIGHT.width,
                height: BOX_HEIGHT,
            },
            Vector2::ZERO,
            0.,
            SHADOW_COLOR,
        );

        let y = if force_pressed {
            y + 2.
        } else if inside {
            y + if pressed { 2. } else { 1. }
        } else {
            y
        };

        tm.draw_texture_pro(
            tex,
            BOX_LEFT,
            Rectangle {
                x,
                y,
                width: BOX_LEFT.width,
                height: BOX_HEIGHT,
            },
            Vector2::ZERO,
            0.,
            Color::WHITE,
        );
        tm.draw_texture_pro(
            tex,
            BOX_RIGHT,
            Rectangle {
                x: x + w - BOX_RIGHT.width,
                y,
                width: BOX_RIGHT.width,
                height: BOX_HEIGHT,
            },
            Vector2::ZERO,
            0.,
            Color::WHITE,
        );
        tm.draw_texture_pro(
            tex,
            BOX_MID,
            Rectangle {
                x: x + BOX_MID.x,
                y,
                width: w - BOX_LEFT.width - BOX_RIGHT.width,
                height: BOX_HEIGHT,
            },
            Vector2::ZERO,
            0.,
            Color::WHITE,
        );

        if text_centered {
            let tw = tm.measure_text(text, 8);
            tm.draw_text(
                text,
                x as i32 + w as i32 / 2 - tw / 2,
                y as i32 + 1,
                8,
                Color::WHITE,
            );
        } else {
            tm.draw_text(
                text,
                (x + BOX_MID.x + 1.) as i32,
                y as i32 + 1,
                8,
                Color::WHITE,
            );
        }
    }
}
