use raylib::ffi::KeyboardKey;

use crate::app::App;

#[cfg(target_family = "wasm")]
const GLSL_VERSION: i32 = 100;
#[cfg(not(target_family = "wasm"))]
const GLSL_VERSION: i32 = 330;

mod app;
mod net;

fn main() {
    let (mut rl, thread) = raylib::init().size(1280, 720).title("wrum").build();
    rl.set_exit_key(Some(KeyboardKey::KEY_NULL));

    let mut a = App::new(&mut rl, &thread);

    while !rl.window_should_close() {
        a.update(&rl);
        let mut d = rl.begin_drawing(&thread);
        a.draw(&mut d);
    }
}
