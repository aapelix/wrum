use raylib::{
    RaylibHandle, RaylibThread,
    color::Color,
    drawing::{RaylibDraw, RaylibDrawHandle, RaylibTextureModeExt},
    math::Vector2,
    texture::RenderTexture2D,
};
use shared::net::ServerMessage;

use crate::{
    net::{Net, SERVER, Status},
    stack::Stack,
};

pub struct App {
    pub canvas: RenderTexture2D,
    stack: Stack,
    net: Net,
}

impl App {
    pub fn new(rl: &mut RaylibHandle, thread: &RaylibThread) -> Self {
        Self {
            canvas: rl.load_render_texture(thread, 360, 180).unwrap(),
            net: Net::new(SERVER).unwrap(),
            stack: Stack::new(rl, thread, "assets/body.png", 10.),
        }
    }

    pub fn update(&mut self, _rl: &RaylibHandle) {
        self.net.poll();
        while let Some(msg) = self.net.next_message() {
            match msg {
                ServerMessage::Welcome { .. } => {}
                _ => {}
            }
        }
        if self.net.status() == Status::Connected {
            // let _ = self.net.send(
            //     Channel::Unreliable,
            //     &ClientMessage::Input {
            //         last_server_tick: 0,
            //     },
            // );
        }
    }

    pub fn draw(&mut self, rl: &mut RaylibHandle, thread: &RaylibThread) {
        let mut tm = rl.begin_texture_mode(thread, &mut self.canvas);
        tm.clear_background(Color::BLACK);
        self.stack.draw(&mut tm, Vector2::zero(), 0.);
    }
}
