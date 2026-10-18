use raylib::{
    drawing::{RaylibDraw, RaylibTextureModeExt},
    ffi::Color,
};

use crate::scene::{Scene, menu::MenuScene};

pub struct ConnectingScene;

impl Scene for ConnectingScene {
    fn draw(&mut self, ctx: &mut crate::ctx::Context) {
        let mut tm = ctx.rl.begin_texture_mode(&ctx.thread, &mut ctx.canvas);
        tm.clear_background(Color::BLACK);
        tm.draw_text("connecting", 130, 80, 10, Color::WHITE);
    }

    fn on_connect(&mut self, ctx: &mut crate::ctx::Context) -> super::Transition {
        super::Transition::Switch(Box::new(MenuScene::new("", ctx)))
    }

    fn blocks_draw(&self) -> bool {
        true
    }
    fn blocks_update(&self) -> bool {
        true
    }
}
