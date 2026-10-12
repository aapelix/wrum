use std::collections::HashMap;

use wrum_core::{PlayerId, physics::Input};

use crate::{camera::Camera, car::Car, ctx, scene::Scene};

pub struct GameScene {
    camera: Camera,
    cars: HashMap<PlayerId, Car>,
    my_id: PlayerId,
}

impl GameScene {
    pub fn new(ctx: &mut ctx::Context, my_id: PlayerId) -> Self {
        Self {
            camera: Camera::default(),
            cars: HashMap::new(),
            my_id,
        }
    }
}

impl Scene for GameScene {
    fn update(&mut self, _ctx: &mut ctx::Context, _dt: f32) -> super::Transition {
        super::Transition::None
    }
    fn draw(&mut self, ctx: &mut ctx::Context) {}

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
    }
}
