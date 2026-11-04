use std::collections::HashMap;

use raylib::{
    RaylibHandle,
    drawing::{RaylibDraw, RaylibTextureMode, RaylibTextureModeExt},
    ffi::{Color, Vector2},
    math::Rectangle,
    texture::{RaylibRenderTexture2D, RenderTexture2D},
};
use wrum_core::{PlayerId, level::Level, sim::Status};

use crate::{camera::Camera, car::Car, ctx, demo::Demo, fx::Fx, map::Map};

type AssetsRef<'a> = &'a mut crate::assets::Assets;

enum Drawable {
    Car(PlayerId),
    Prop,
}

pub struct Backdrop {
    demo: Demo,
    camera: Camera,
    map: Map,
    fx: Fx,
    shadow_canvas: RenderTexture2D,
    cars: HashMap<PlayerId, Car>,
    clock: f32,
}

impl Backdrop {
    pub fn new(ctx: &mut ctx::Context, level_path: &str) -> Self {
        let level = Level::load(level_path).unwrap();
        let map = Map::load(ctx, &level_path.replace("_bot.json", ".png"), level_path, 1);
        let fx = Fx::new(ctx, Vector2::zero(), map.size);
        let shadow_canvas = ctx.rl.load_render_texture(ctx.thread, 320, 180).unwrap();
        Self {
            demo: Demo::new(level),
            camera: Camera::default(),
            map,
            fx,
            shadow_canvas,
            cars: HashMap::new(),
            clock: 0.0,
        }
    }

    pub fn update(&mut self, ctx: &mut ctx::Context, dt: f32) {
        self.clock += dt;
        self.demo.update(dt.min(0.25));

        let c = self.demo.w.cars[0];
        let (pos, rot, z) = self.demo.view();
        let id: PlayerId = 0;
        let car = self
            .cars
            .entry(id)
            .or_insert_with(|| Car::new(ctx, Vector2 { x: pos.x, y: pos.y }, rot.to_degrees(), 0.));
        car.p = Vector2 { x: pos.x, y: pos.y };
        car.a = rot.to_degrees();
        car.z = z;
        car.invuln = c.invuln;
        if c.status == Status::Drowning && car.status != Status::Drowning {
            car.sunk_at = self.clock;
        }
        car.status = c.status;
        car.drift_dir = c.drift_dir;
        car.tier = c.tier();
        car.charge = c.charge;
        car.boost = c.boost;
        car.boost_lvl = c.boost_lvl;
        let (cp, ca, cb) = (car.p, car.a, car.boost);

        let map = &self.map;
        self.fx
            .update(dt, &self.cars, self.demo.level(), &|p| map.is_water(p));
        self.camera.set_boost(cb);
        self.camera.update(cp, ca, dt);
    }
    pub fn prepare(&mut self, ctx: &mut ctx::Context) {
        self.fx.flush(ctx);
        let mut tm = ctx
            .rl
            .begin_texture_mode(&ctx.thread, &mut self.shadow_canvas);
        tm.clear_background(Color::BLANK);
        self.map.draw_shadows(ctx.assets, &mut tm, &self.camera);
        for car in self.cars.values() {
            car.draw_shadow(ctx.assets, &mut tm, &self.camera);
        }
    }
    pub fn draw(&mut self, assets: AssetsRef, tm: &mut RaylibTextureMode<'_, '_, RaylibHandle>) {
        self.map.draw_layers(assets, tm, &self.camera);
        self.fx.draw_marks(tm, &self.camera);
        tm.draw_texture_rec(
            self.shadow_canvas.texture(),
            Rectangle::new(0., 0., 320., -180.),
            Vector2::zero(),
            Color::new(255, 255, 255, 180),
        );
        self.fx.draw_trails(tm, &self.camera);
        let map = &self.map;
        self.fx.draw_ripples(tm, &self.camera, &|p| map.is_water(p));

        let mut drawables: Vec<(f32, usize, Drawable)> = Vec::new();
        for (i, (id, car)) in self.cars.iter().enumerate() {
            let (pos, _) = self.camera.apply(car.p, car.a);
            drawables.push((pos.y, i, Drawable::Car(*id)));
        }
        for (i, prop) in self.map.props.iter().enumerate() {
            let (pos, _) = self.camera.apply(prop.pos, prop.a);
            drawables.push((pos.y, i, Drawable::Prop));
        }
        drawables.sort_by(|a, b| {
            a.0.partial_cmp(&b.0)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(a.1.cmp(&b.1))
        });
        for (_, i, d) in drawables {
            match d {
                Drawable::Car(id) => {
                    let car = self.cars.get(&id).unwrap();
                    let (map, cam) = (&mut self.map, &self.camera);
                    car.draw(
                        assets,
                        tm,
                        cam,
                        self.clock,
                        |tm, pos, depth| {
                            map.draw_water_overlay(assets, tm, cam, pos, depth);
                        },
                        false,
                    );
                }
                Drawable::Prop => {
                    self.map.draw_prop(assets, tm, i, &self.camera);
                }
            }
        }

        self.fx.draw_flames(tm, &self.camera);
        self.fx.draw_bits(tm, &self.camera);
        self.fx.draw_puffs(tm, &self.camera);
        let map = &self.map;
        self.fx
            .draw_droplets(tm, &self.camera, &|p| map.is_water(p));

        tm.draw_rectangle(0, 0, 320, 180, Color::new(0, 0, 0, 150));
    }
}
