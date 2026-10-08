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
    let mut app = App::new();
    while !app.should_close() {
        app.update();
        app.draw();
    }
}
