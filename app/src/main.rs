use raylib::{color::Color, drawing::RaylibDraw, ffi::KeyboardKey};

use crate::app::App;

// #[cfg(target_family = "wasm")]
// const GLSL_VERSION: i32 = 100;
// #[cfg(not(target_family = "wasm"))]
// const GLSL_VERSION: i32 = 330;

mod app;
mod net;
mod stack;

fn main() {
    let (mut rl, thread) = raylib::init().size(1280, 720).title("wrum").build();
    rl.set_exit_key(Some(KeyboardKey::KEY_NULL));

    let mut a = App::new(&mut rl, &thread);

    while !rl.window_should_close() {
        a.update(&rl);
        a.draw(&mut rl, &thread);
        let mut d = rl.begin_drawing(&thread);
        d.clear_background(Color::BLACK);
    }
}
