use raylib::{color::Color, drawing::RaylibDraw, ffi::KeyboardKey};

#[cfg(target_family = "wasm")]
const GLSL_VERSION: i32 = 100;
#[cfg(not(target_family = "wasm"))]
const GLSL_VERSION: i32 = 330;

mod app;
mod net;

fn main() {
    let (mut rl, thread) = raylib::init().size(1280, 720).title("wrum").build();
    rl.set_exit_key(Some(KeyboardKey::KEY_NULL));

    while !rl.window_should_close() {
        let mut d = rl.begin_drawing(&thread);

        d.clear_background(Color::WHITE);
        d.draw_text("Hello, world!", 12, 12, 20, Color::BLACK);
    }
}
