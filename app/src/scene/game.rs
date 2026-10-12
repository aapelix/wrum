use std::collections::HashMap;

use raylib::{
    RaylibHandle,
    drawing::{RaylibDraw, RaylibTextureModeExt},
    ffi::{Color, KeyboardKey, Vector2},
};
use wrum_core::{
    PlayerId,
    physics::{CarState, Input},
};
use wrum_net::{
    Channel,
    proto::{ClientMsg, ServerMsg},
};

use crate::{camera::Camera, car::Car, ctx, scene::Scene};

pub struct GameScene {
    camera: Camera,
    cars: HashMap<PlayerId, Car>,
    my_id: PlayerId,
}

impl GameScene {
    pub fn new(
        ctx: &mut ctx::Context,
        my_id: PlayerId,
        spawn: &CarState,
        existing: &Vec<(PlayerId, CarState)>,
    ) -> Self {
        let mut s = Self {
            camera: Camera::default(),
            cars: HashMap::new(),
            my_id,
        };

        s.ensure_car(ctx, my_id, spawn);
        for (id, state) in existing {
            s.ensure_car(ctx, *id, state);
        }

        s
    }

    fn ensure_car(&mut self, ctx: &mut ctx::Context, id: PlayerId, spawn: &CarState) {
        self.cars.entry(id).or_insert_with(|| {
            Car::new(
                ctx,
                Vector2 {
                    x: spawn.pos.x,
                    y: spawn.pos.y,
                },
                spawn.rot.to_degrees(),
                0.,
                0.,
            )
        });
    }
    fn sim_tick(&mut self, ctx: &mut ctx::Context) {
        let input = sample_input(ctx.rl);
        let msg = ClientMsg::Input { input };
        if let Err(e) = ctx.net.send(Channel::Unreliable, &msg) {
            eprintln!("failed to send input: {e:#?}")
        };
    }
    fn apply_snapshot(
        &mut self,
        _ctx: &mut ctx::Context,
        _tick: u32,
        cars: &Vec<(PlayerId, CarState)>,
    ) {
        for (id, state) in cars {
            if let Some(car) = self.cars.get_mut(id) {
                car.p = Vector2 {
                    x: state.pos.x,
                    y: state.pos.y,
                };
                car.a = state.rot.to_degrees();
            }
        }
    }
}

impl Scene for GameScene {
    fn fixed_update(&mut self, ctx: &mut ctx::Context) -> super::Transition {
        self.sim_tick(ctx);
        super::Transition::None
    }
    fn update(&mut self, _ctx: &mut ctx::Context, dt: f32, _alpha: f32) -> super::Transition {
        // interp

        let me = self.cars.get(&self.my_id).unwrap();
        self.camera.update(me.p, me.a, dt);

        super::Transition::None
    }
    fn draw(&mut self, ctx: &mut ctx::Context) {
        let mut tm = ctx.rl.begin_texture_mode(ctx.thread, ctx.canvas);
        tm.clear_background(Color::BLACK);
        for car in self.cars.values() {
            car.draw(&mut tm, &self.camera);
        }
    }

    fn on_message(
        &mut self,
        ctx: &mut ctx::Context,
        msg: &wrum_net::proto::ServerMsg,
    ) -> super::Transition {
        match msg {
            ServerMsg::Snapshot { tick, cars } => self.apply_snapshot(ctx, *tick, cars),
            ServerMsg::PlayerJoined { id, name: _, spawn } => {
                self.ensure_car(ctx, *id, spawn);
            }
            _ => {}
        }
        super::Transition::None
    }

    fn blocks_draw(&self) -> bool {
        true
    }
    fn blocks_update(&self) -> bool {
        true
    }
}

fn sample_input(rl: &RaylibHandle) -> Input {
    Input {
        seq: 0,
        steer: rl.is_key_down(KeyboardKey::KEY_D) as i8 - rl.is_key_down(KeyboardKey::KEY_A) as i8,
        throttle: rl.is_key_down(KeyboardKey::KEY_W) as i8
            - rl.is_key_down(KeyboardKey::KEY_S) as i8,
        drift: rl.is_key_down(KeyboardKey::KEY_SPACE),
    }
}
