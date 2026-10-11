use raylib::{
    drawing::{RaylibDraw, RaylibTextureModeExt},
    ffi::{Color, Vector2},
    math::Rectangle,
    texture::RaylibRenderTexture2D,
    trace_log,
};

use crate::net::Net;

mod game_loop;
mod net;

fn main() -> anyhow::Result<()> {
    let (mut rl, thread) = raylib::init().size(1280, 720).title("wrum").build();
    let mut canvas = rl.load_render_texture(&thread, 320, 180)?;

    let mut net = Net::connect("https://127.0.0.1:7778")?;

    game_loop::run(rl, thread, 0, move |rl, thread| {
        while let Ok(Some((ch, bytes))) = net.poll() {
            let (_, _) = (ch, bytes);
        }

        let ww = rl.get_screen_width() as f32;
        let wh = rl.get_screen_height() as f32;

        {
            let mut tm = rl.begin_texture_mode(thread, &mut canvas);
            tm.clear_background(Color::WHITE);
            tm.draw_text("hello worlda", 12, 12, 20, Color::BLACK);
            tm.draw_fps(12, 30);
        }

        let mut d = rl.begin_drawing(thread);
        d.clear_background(Color::BLACK);
        d.draw_texture_pro(
            canvas.texture(),
            Rectangle::new(0., 0., 320., -180.),
            Rectangle::new(0., 0., ww, wh),
            Vector2::zero(),
            0.,
            Color::WHITE,
        );
    });

    Ok(())
}
