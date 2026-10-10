use raylib::{
    drawing::{RaylibDraw, RaylibTextureModeExt},
    ffi::{Color, Vector2},
    math::Rectangle,
    texture::RaylibRenderTexture2D,
};

mod game_loop;

fn main() -> anyhow::Result<()> {
    let (mut rl, thread) = raylib::init().size(1280, 720).title("wrum").build();
    let mut canvas = rl.load_render_texture(&thread, 320, 180)?;

    game_loop::run(rl, thread, 0, move |rl, thread| {
        let ww = rl.get_screen_width() as f32;
        let wh = rl.get_screen_height() as f32;

        {
            let mut tm = rl.begin_texture_mode(thread, &mut canvas);
            tm.clear_background(Color::WHITE);
            tm.draw_text("hello world", 12, 12, 20, Color::BLACK);
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
