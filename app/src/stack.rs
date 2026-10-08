use raylib::{
    RaylibHandle,
    color::Color,
    drawing::{RaylibDraw, RaylibTextureMode, RaylibTextureModeExt},
    ffi::TextureFilter,
    math::{Rectangle, Vector2},
    texture::{RaylibRenderTexture2D, RaylibTexture2D, RenderTexture2D, Texture2D},
};

use crate::ctx::Context;

pub struct Stack {
    tex: Texture2D,
    shadow: RenderTexture2D,
    center: Vector2,

    sw: f32,
    h: f32,
    c: i32,
}

impl Stack {
    pub fn new(ctx: &mut Context, path: &str, sw: f32) -> Self {
        let tex = ctx.rl.load_texture(&ctx.thread, path).unwrap();
        tex.set_texture_filter(&ctx.thread, TextureFilter::TEXTURE_FILTER_POINT);

        let w = tex.width();
        let h = tex.height();
        let c = w / sw as i32;

        let mut shadow = ctx
            .rl
            .load_render_texture(&ctx.thread, w as u32, h as u32)
            .unwrap();
        {
            let mut tm = ctx.rl.begin_texture_mode(&ctx.thread, &mut shadow);
            tm.clear_background(Color::BLANK);

            for i in 0..c {
                tm.draw_texture_rec(
                    &tex,
                    Rectangle {
                        x: i as f32 * sw,
                        y: 0.,
                        width: sw,
                        height: h as f32,
                    },
                    Vector2::zero(),
                    Color::BLACK,
                );
            }
        }

        Self {
            tex,
            shadow,
            center: Vector2 {
                x: sw / 2.,
                y: h as f32 / 2.,
            },
            sw,
            h: h as f32,
            c,
        }
    }
    pub fn draw(&self, tm: &mut RaylibTextureMode<'_, RaylibHandle>, pos: Vector2, a: f32) {
        for i in 0..self.c {
            let i = i as f32;
            let src = Rectangle {
                x: i * self.sw,
                y: 0.,
                width: self.sw,
                height: self.h,
            };
            let dst = Rectangle {
                x: pos.x,
                y: pos.y - i,
                width: self.sw,
                height: self.h,
            };
            tm.draw_texture_pro(&self.tex, src, dst, self.center, a, Color::WHITE);
        }
    }
    pub fn draw_shadow(&self, tm: &mut RaylibTextureMode<'_, RaylibHandle>, pos: Vector2, a: f32) {
        tm.draw_texture_pro(
            &self.shadow.texture(),
            Rectangle {
                x: 0.,
                y: 0.,
                width: self.sw,
                height: self.h,
            },
            Rectangle {
                x: pos.x,
                y: pos.y,
                width: self.sw,
                height: self.h,
            },
            self.center,
            a,
            Color::WHITE,
        );
    }
}
