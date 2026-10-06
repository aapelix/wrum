use shared::net::{DisconnectReason, ServerMessage};

use crate::ctx::Context;

pub mod general;

pub trait Scene {
    fn update(&mut self, ctx: &mut Context, dt: f32) -> Transition;
    fn draw(&mut self, ctx: &mut Context);

    fn on_connect(&mut self, _ctx: &mut Context) -> Transition {
        Transition::None
    }
    fn on_disconnect(&mut self, _ctx: &mut Context, _reason: DisconnectReason) -> Transition {
        Transition::None
    }
    fn on_message(&mut self, _ctx: &mut Context, _msg: ServerMessage) -> Transition {
        Transition::None
    }

    fn blocks_update(&self) -> bool;
    fn blocks_draw(&self) -> bool;

    fn enter(&mut self, _ctx: &mut Context) {}
    fn exit(&mut self, _ctx: &mut Context) {}
}

pub enum Transition {
    None,
    Push(Box<dyn Scene>),
    Pop,
    Switch(Box<dyn Scene>),
}

pub struct SceneManager {
    stack: Vec<Box<dyn Scene>>,
}

impl SceneManager {
    pub fn new(mut init: impl Scene + 'static, ctx: &mut Context) -> Self {
        init.enter(ctx);
        Self {
            stack: vec![Box::new(init)],
        }
    }

    pub fn update(&mut self, ctx: &mut Context, dt: f32) {
        let mut t = Transition::None;

        for scene in self.stack.iter_mut().rev() {
            let res = scene.update(ctx, dt);
            if !matches!(res, Transition::None) {
                t = res;
            }
            if scene.blocks_update() {
                break;
            }
        }

        self.apply(ctx, t);
    }
    pub fn draw(&mut self, ctx: &mut Context) {
        let mut start = self.stack.len().saturating_sub(1);
        while start > 0 && !self.stack[start].blocks_draw() {
            start -= 1;
        }
        for scene in &mut self.stack[start..] {
            scene.draw(ctx);
        }
    }

    pub fn on_connect(&mut self, ctx: &mut Context) {
        if let Some(last) = self.stack.last_mut() {
            let t = last.on_connect(ctx);
            self.apply(ctx, t);
        }
    }
    pub fn on_disconnect(&mut self, ctx: &mut Context, reason: DisconnectReason) {
        if let Some(last) = self.stack.last_mut() {
            let t = last.on_disconnect(ctx, reason);
            self.apply(ctx, t);
        }
    }
    pub fn on_message(&mut self, ctx: &mut Context, msg: ServerMessage) {
        if let Some(last) = self.stack.last_mut() {
            let t = last.on_message(ctx, msg);
            self.apply(ctx, t);
        }
    }

    fn apply(&mut self, ctx: &mut Context, t: Transition) {
        match t {
            Transition::None => {}
            Transition::Push(mut scene) => {
                scene.enter(ctx);
                self.stack.push(scene);
            }
            Transition::Pop => {
                if let Some(mut scene) = self.stack.pop() {
                    scene.exit(ctx);
                }
            }
            Transition::Switch(mut scene) => {
                while let Some(mut scene) = self.stack.pop() {
                    scene.exit(ctx);
                }
                scene.enter(ctx);
                self.stack.push(scene);
            }
        }
    }
}
