#[cfg(target_arch = "wasm32")]
extern crate console_error_panic_hook;

mod app;
mod assets;
mod backdrop;
mod camera;
mod car;
mod ctx;
mod demo;
mod fx;
mod game_loop;
mod map;
mod net;
mod scene;
mod sim;
mod stack;
mod ui;

#[cfg(target_family = "wasm")]
pub const GLSL_VERSION: i32 = 100;
#[cfg(not(target_family = "wasm"))]
pub const GLSL_VERSION: i32 = 330;

fn main() -> anyhow::Result<()> {
    let (mut rl, thread) = raylib::init().size(1280, 720).title("wrum").build();

    #[cfg(target_arch = "wasm32")]
    console_error_panic_hook::set_once();
  
    let mut app = app::App::new(&mut rl, &thread)?;

    game_loop::run(rl, thread, 0, move |rl, thread| {
        app.update(rl, thread);
        app.draw(rl, thread);
    });

    Ok(())
}
