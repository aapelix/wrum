use raylib::{
    color::Color,
    drawing::RaylibDraw,
    ffi::KeyboardKey,
    math::{Rectangle, Vector2},
    texture::RaylibRenderTexture2D,
};

use crate::{
    ctx::Context,
    net::Status,
    scene::{SceneManager, general::ConnectingScene},
};

pub struct App {
    scenes: SceneManager,
    pub ctx: Context,
}

impl App {
    pub fn new() -> Self {
        let (rl, thread) = raylib::init().size(1280, 720).title("wrum").build();
        let mut ctx = Context::new(rl, thread);
        ctx.rl.set_exit_key(Some(KeyboardKey::KEY_NULL));

        let scenes = SceneManager::new(ConnectingScene, &mut ctx);

        Self { scenes, ctx }
    }

    pub fn should_close(&self) -> bool {
        self.ctx.rl.window_should_close()
    }

    pub fn update(&mut self) {
        let dt = self.ctx.rl.get_frame_time();

        self.ctx.net.poll();
        let ns = self.ctx.net.status();
        if self.ctx.last_status != Some(ns) {
            match ns {
                Status::Connected => {
                    self.scenes.on_connect(&mut self.ctx);
                }
                Status::Disconnected(reason) => {
                    self.scenes.on_disconnect(&mut self.ctx, reason);
                }
                _ => {}
            }
            self.ctx.last_status = Some(ns);
        }
        while let Some(msg) = self.ctx.net.next_message() {
            self.scenes.on_message(&mut self.ctx, msg);
        }

        self.scenes.update(&mut self.ctx, dt);
    }
    pub fn draw(&mut self) {
        self.scenes.draw(&mut self.ctx);

        let ww = self.ctx.rl.get_screen_width() as f32;
        let wh = self.ctx.rl.get_screen_height() as f32;

        let mut d = self.ctx.rl.begin_drawing(&self.ctx.thread);
        d.clear_background(Color::BLACK);

        d.draw_texture_pro(
            &self.ctx.canvas.texture(),
            Rectangle::new(0., 0., 320., -180.),
            Rectangle::new(0., 0., ww, wh),
            Vector2::zero(),
            0.,
            Color::WHITE,
        );
    }
}
