use raylib::{
    drawing::{RaylibDraw, RaylibTextureModeExt},
    ffi::Color,
};

use crate::scene::{Scene, auth::AuthScene};

pub struct ConnectingScene;

impl Scene for ConnectingScene {
    fn draw(&mut self, ctx: &mut crate::ctx::Context) {
        let mut tm = ctx.rl.begin_texture_mode(&ctx.thread, &mut ctx.canvas);
        tm.clear_background(Color::BLACK);
        tm.draw_text("connecting", 130, 80, 10, Color::WHITE);
    }

    fn on_connect(&mut self, ctx: &mut crate::ctx::Context) -> super::Transition {
        super::Transition::Switch(Box::new(AuthScene::new(ctx)))
    }

    fn blocks_draw(&self) -> bool {
        true
    }
    fn blocks_update(&self) -> bool {
        true
    }
}

pub struct DisconnectedScene;

impl Scene for DisconnectedScene {
    fn enter(&mut self, ctx: &mut crate::ctx::Context) {
        ctx.net.reconnect().unwrap();
    }

    fn draw(&mut self, ctx: &mut crate::ctx::Context) {
        let mut tm = ctx.rl.begin_texture_mode(&ctx.thread, &mut ctx.canvas);
        tm.clear_background(Color::BLACK);
        let w = tm.measure_text("disconnected, refresh", 10);
        tm.draw_text("disconnected, refresh", 160 - w / 2, 80, 10, Color::RED);
    }

    fn on_connect(&mut self, ctx: &mut crate::ctx::Context) -> super::Transition {
        super::Transition::Switch(Box::new(AuthScene::new(ctx)))
    }

    fn blocks_draw(&self) -> bool {
        true
    }
    fn blocks_update(&self) -> bool {
        true
    }
}
