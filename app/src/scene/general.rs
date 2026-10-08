use raylib::{
    color::Color,
    drawing::{RaylibDraw, RaylibTextureModeExt},
};

use crate::scene::{Scene, menu::MenuScene};

pub struct ConnectingScene;
pub struct PauseScene;

impl Scene for ConnectingScene {
    fn update(&mut self, _ctx: &mut crate::ctx::Context, _dt: f32) -> super::Transition {
        super::Transition::None
    }
    fn draw(&mut self, ctx: &mut crate::ctx::Context) {
        let mut tm = ctx.rl.begin_texture_mode(&ctx.thread, &mut ctx.canvas);
        tm.clear_background(Color::BLACK);
        tm.draw_text("connecting", 125, 80, 10, Color::WHITE);
    }

    fn on_connect(&mut self, _ctx: &mut crate::ctx::Context) -> super::Transition {
        super::Transition::Switch(Box::new(MenuScene::default()))
    }

    fn blocks_draw(&self) -> bool {
        true
    }
    fn blocks_update(&self) -> bool {
        true
    }
}

impl Scene for PauseScene {
    fn update(&mut self, ctx: &mut crate::ctx::Context, _dt: f32) -> super::Transition {
        if ctx.rl.is_key_pressed(raylib::ffi::KeyboardKey::KEY_ESCAPE) {
            return super::Transition::Pop;
        }
        super::Transition::None
    }
    fn draw(&mut self, ctx: &mut crate::ctx::Context) {
        let mut tm = ctx.rl.begin_texture_mode(&ctx.thread, &mut ctx.canvas);
        tm.draw_rectangle(0, 0, 320, 180, Color::new(0, 0, 0, 120));
    }

    fn blocks_draw(&self) -> bool {
        false
    }
    fn blocks_update(&self) -> bool {
        true
    }
}
