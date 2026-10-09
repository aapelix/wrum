use raylib::{
    RaylibHandle, RaylibThread,
    color::Color,
    drawing::RaylibDraw,
    math::{Rectangle, Vector2},
    texture::RaylibRenderTexture2D,
};
use shared::sim::TICK_RATE;

use crate::{
    ctx::State,
    net::Status,
    scene::{SceneManager, general::ConnectingScene},
};

pub struct App {
    scenes: SceneManager,
    state: State,
    stepper: Stepper,
}

impl App {
    pub fn new(rl: &mut RaylibHandle, thread: &RaylibThread) -> Self {
        let mut state = State::new(rl, thread);
        let scenes = SceneManager::new(ConnectingScene, &mut state.ctx(rl, thread));
        let stepper = Stepper::new(TICK_RATE as f32);

        Self {
            scenes,
            state,
            stepper,
        }
    }
    pub fn frame(&mut self, rl: &mut RaylibHandle, thread: &RaylibThread) {
        let dt = rl.get_frame_time();
        let mut ctx = self.state.ctx(rl, thread);
        ctx.net.poll();
        let ns = ctx.net.status();
        if *ctx.last_status != Some(ns) {
            match ns {
                Status::Connected => {
                    self.scenes.on_connect(&mut ctx);
                }
                Status::Disconnected(reason) => {
                    self.scenes.on_disconnect(&mut ctx, reason);
                }
                _ => {}
            }
            *ctx.last_status = Some(ns);
        }
        while let Some(msg) = ctx.net.next_message() {
            self.scenes.on_message(&mut ctx, msg);
        }

        let (ticks, alpha) = self.stepper.advance(dt);
        for _ in 0..ticks {
            self.scenes.fixed_update(&mut ctx);
        }
        self.scenes.update(&mut ctx, dt, alpha);

        self.scenes.draw(&mut ctx);

        let ww = ctx.rl.get_screen_width() as f32;
        let wh = ctx.rl.get_screen_height() as f32;

        let mut d = ctx.rl.begin_drawing(&ctx.thread);
        d.clear_background(Color::BLACK);

        d.draw_texture_pro(
            &ctx.canvas.texture(),
            Rectangle::new(0., 0., 320., -180.),
            Rectangle::new(0., 0., ww, wh),
            Vector2::zero(),
            0.,
            Color::WHITE,
        );
    }
}

pub struct Stepper {
    acc: f32,
    dt: f32,
}
impl Stepper {
    pub fn new(hz: f32) -> Self {
        Self {
            acc: 0.,
            dt: 1. / hz,
        }
    }
    pub fn advance(&mut self, frame_dt: f32) -> (u32, f32) {
        self.acc = (self.acc + frame_dt).min(0.1);
        let mut n = 0;
        while self.acc >= self.dt && n < 3 {
            self.acc -= self.dt;
            n += 1;
        }
        (n, self.acc / self.dt)
    }
}
