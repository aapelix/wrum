use raylib::{
    RaylibHandle, RaylibThread,
    drawing::RaylibDraw,
    ffi::{Color, Vector2},
    math::Rectangle,
    texture::RaylibRenderTexture2D,
};
use wrum_net::proto::{PROTO_VERSION, TICK_RATE};

use crate::{
    ctx::State,
    net::NetEvent,
    scene::{SceneManager, basic::ConnectingScene},
};

pub struct App {
    state: State,
    scenes: SceneManager,
    events: Vec<NetEvent>,
    stepper: Stepper,
}

impl App {
    pub fn new(rl: &mut RaylibHandle, thread: &RaylibThread) -> anyhow::Result<App> {
        let mut state = State::new(rl, thread)?;
        let mut ctx = state.ctx(rl, thread);
        let scenes = SceneManager::new(ConnectingScene, &mut ctx);
        let stepper = Stepper::new(TICK_RATE as f32);

        Ok(App {
            state,
            scenes,
            events: Vec::new(),
            stepper,
        })
    }
    pub fn update(&mut self, rl: &mut RaylibHandle, thread: &RaylibThread) {
        let dt = rl.get_frame_time();
        let mut ctx = self.state.ctx(rl, thread);

        self.events.clear();
        ctx.net.pump(&mut self.events).unwrap();
        for e in &self.events {
            match e {
                NetEvent::Connected => {
                    self.scenes.on_connect(&mut ctx);
                    ctx.net
                        .send(&wrum_net::proto::ClientMsg::Hello {
                            version: PROTO_VERSION,
                            name: "test".into(),
                        })
                        .unwrap();
                }
                NetEvent::Disconnected(_err) => {
                    self.scenes.on_disconnect(&mut ctx);
                }
                NetEvent::Message(msg) => {
                    self.scenes.on_message(&mut ctx, msg);
                }
            }
        }

        let (ticks, alpha) = self.stepper.advance(dt);
        for _ in 0..ticks {
            self.scenes.fixed_update(&mut ctx);
        }
        self.scenes.update(&mut ctx, dt, alpha);
    }
    pub fn draw(&mut self, rl: &mut RaylibHandle, thread: &RaylibThread) {
        let mut ctx = self.state.ctx(rl, thread);
        self.scenes.draw(&mut ctx);

        let ww = ctx.rl.get_screen_width() as f32;
        let wh = ctx.rl.get_screen_height() as f32;

        let mut d = ctx.rl.begin_drawing(thread);
        d.clear_background(Color::BLACK);
        d.draw_texture_pro(
            ctx.canvas.texture(),
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
