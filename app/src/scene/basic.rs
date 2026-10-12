use raylib::{
    drawing::{RaylibDraw, RaylibTextureModeExt},
    ffi::Color,
};
use wrum_net::proto::{ClientMsg, ServerMsg};

use crate::scene::{Scene, game::GameScene};

pub struct ConnectingScene;

impl Scene for ConnectingScene {
    fn draw(&mut self, ctx: &mut crate::ctx::Context) {
        let mut tm = ctx.rl.begin_texture_mode(&ctx.thread, &mut ctx.canvas);
        tm.clear_background(Color::BLACK);
        tm.draw_text("connecting", 125, 80, 10, Color::WHITE);
    }

    fn on_connect(&mut self, ctx: &mut crate::ctx::Context) -> super::Transition {
        //super::Transition::Switch(Box::new(MenuScene::default()))
        let _ = ctx
            .net
            .send(wrum_net::Channel::Reliable, &ClientMsg::JoinPublic);
        crate::scene::Transition::None
    }
    fn on_message(
        &mut self,
        ctx: &mut crate::ctx::Context,
        msg: &wrum_net::proto::ServerMsg,
    ) -> super::Transition {
        match msg {
            ServerMsg::Welcome {
                your_id,
                spawn,
                existing,
            } => {
                return super::Transition::Switch(Box::new(GameScene::new(
                    ctx, *your_id, spawn, existing,
                )));
            }
            _ => {}
        }
        super::Transition::None
    }

    fn blocks_draw(&self) -> bool {
        true
    }
    fn blocks_update(&self) -> bool {
        true
    }
}
