use raylib::{
    color::Color,
    drawing::{RaylibDraw, RaylibTextureModeExt},
    ffi::{KeyboardKey, MouseButton},
    math::Rectangle,
};
use shared::net::{Channel, ClientMessage, ServerMessage};

use crate::scene::Scene;

pub const QUICK_RECT: Rectangle = Rectangle {
    x: 20. * 4.,
    y: 50. * 4.,
    width: 120. * 4.,
    height: 16. * 4.,
};
pub const CREATE_RECT: Rectangle = Rectangle {
    x: 20. * 4.,
    y: 68. * 4.,
    width: 120. * 4.,
    height: 16. * 4.,
};
pub const JOIN_RECT: Rectangle = Rectangle {
    x: 20. * 4.,
    y: 86. * 4.,
    width: 120. * 4.,
    height: 16. * 4.,
};

#[derive(Default)]
pub struct MenuScene {
    status: String,
    code: String,
}

impl Scene for MenuScene {
    fn update(&mut self, ctx: &mut crate::ctx::Context, _dt: f32) -> super::Transition {
        let mouse = ctx.rl.get_mouse_position();
        if ctx
            .rl
            .is_mouse_button_pressed(MouseButton::MOUSE_BUTTON_LEFT)
        {
            if QUICK_RECT.check_collision_point_rec(mouse) {
                ctx.net
                    .send(Channel::ReliableUnordered, &ClientMessage::Quickplay)
                    .unwrap()
            } else if CREATE_RECT.check_collision_point_rec(mouse) {
                ctx.net
                    .send(Channel::ReliableUnordered, &ClientMessage::CreateLobby)
                    .unwrap()
            } else if JOIN_RECT.check_collision_point_rec(mouse) && self.code.len() != 0 {
                ctx.net
                    .send(
                        Channel::ReliableUnordered,
                        &ClientMessage::JoinLobby {
                            code: self.code.clone(),
                        },
                    )
                    .unwrap()
            }
        }

        if let Some(c) = ctx.rl.get_char_pressed() {
            if c.is_ascii_alphanumeric() {
                self.code.push(c);
            }
        }
        if ctx.rl.is_key_pressed(KeyboardKey::KEY_BACKSPACE) {
            self.code.pop();
        }

        super::Transition::None
    }
    fn draw(&mut self, ctx: &mut crate::ctx::Context) {
        let mut tm = ctx.rl.begin_texture_mode(&ctx.thread, &mut ctx.canvas);
        tm.clear_background(Color::BLACK);
        tm.draw_text("wrum", 20, 10, 20, Color::WHITE);
        if self.status.len() != 0 {
            tm.draw_text(&self.status, 20, 30, 10, Color::RED);
        }
        tm.draw_rectangle_lines(20, 50, 120, 16, Color::WHITE);
        tm.draw_text("Quickplay", 28, 52, 8, Color::WHITE);
        tm.draw_rectangle_lines(20, 68, 120, 16, Color::WHITE);
        tm.draw_text("Create private", 28, 70, 8, Color::WHITE);
        tm.draw_rectangle_lines(20, 86, 120, 16, Color::WHITE);
        tm.draw_text("Join", 28, 88, 8, Color::WHITE);
        tm.draw_text(&self.code, 150, 88, 8, Color::WHITE);
    }

    fn on_message(
        &mut self,
        _ctx: &mut crate::ctx::Context,
        msg: shared::net::ServerMessage,
    ) -> super::Transition {
        match msg {
            ServerMessage::Welcome { .. } => {
                // switch to game scene
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
