mod app;
mod camera;
mod car;
mod ctx;
mod fx;
mod game_loop;
mod map;
mod net;
mod scene;
mod stack;

#[cfg(target_family = "wasm")]
pub const GLSL_VERSION: i32 = 100;
#[cfg(not(target_family = "wasm"))]
pub const GLSL_VERSION: i32 = 330;

fn main() -> anyhow::Result<()> {
    let (mut rl, thread) = raylib::init().size(1280, 720).title("wrum").build();

    let mut app = app::App::new(&mut rl, &thread)?;

    game_loop::run(rl, thread, 0, move |rl, thread| {
        app.update(rl, thread);
        app.draw(rl, thread);
    });

    Ok(())
}
