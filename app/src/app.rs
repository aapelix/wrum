use raylib::{
    RaylibHandle, RaylibThread,
    color::Color,
    drawing::{RaylibDraw, RaylibDrawHandle},
    texture::RenderTexture2D,
};
use shared::net::ServerMessage;

use crate::net::{Net, SERVER, Status};

pub struct App {
    canvas: RenderTexture2D,
    net: Net,
}

impl App {
    pub fn new(rl: &mut RaylibHandle, thread: &RaylibThread) -> Self {
        Self {
            canvas: rl.load_render_texture(thread, 360, 180).unwrap(),
            net: Net::new(SERVER).unwrap(),
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

    pub fn draw(&self, d: &mut RaylibDrawHandle) {
        d.clear_background(Color::WHITE);
        let s = match self.net.status() {
            Status::Connecting => "connecting...",
            Status::Connected => "connected",
            Status::Disconnected(_) => "disconnected",
        };
        d.draw_text(s, 12, 12, 20, Color::BLACK);
    }
}
