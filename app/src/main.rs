use raylib::ffi::KeyboardKey;

use crate::app::App;

#[cfg(target_family = "wasm")]
pub const GLSL_VERSION: i32 = 100;
#[cfg(not(target_family = "wasm"))]
pub const GLSL_VERSION: i32 = 330;

mod app;
mod camera;
mod ctx;
mod entity;
mod map;
mod net;
mod scene;
mod stack;

fn main() {
    let (mut rl, thread) = raylib::init().size(1280, 720).title("wrum").build();
    rl.set_exit_key(Some(KeyboardKey::KEY_NULL));
    let mut app = App::new(&mut rl, &thread);
    raylib::core::game_loop::run(rl, thread, 0, move |rl, thread| {
        app.frame(rl, thread);
    });
}
