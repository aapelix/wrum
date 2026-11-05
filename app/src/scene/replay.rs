use std::{cmp::Ordering, collections::HashMap};

use raylib::{
    drawing::{RaylibDraw, RaylibTextureModeExt},
    ffi::{Color, KeyboardKey, Vector2},
    math::Rectangle,
    texture::{RaylibRenderTexture2D, RenderTexture2D},
};
use wrum_core::{
    PlayerId,
    level::Level,
    replay::{Replay, ReplayPlayback},
    sim::{DT, MAX_CARS, Status, grid},
};

use crate::{
    camera::Camera,
    car::Car,
    ctx::Context,
    fx::Fx,
    map::Map as VisualMap,
    scene::{
        Scene, Transition,
        game::{Pose, apply_pose, format_time},
        menu::MenuScene,
    },
};

pub struct ReplayScene {
    playback: ReplayPlayback,

    level: Level,
    sim_map: grid::Map,
    map: VisualMap,

    camera: Camera,
    shadow_canvas: RenderTexture2D,
    fx: Fx,

    cars: HashMap<PlayerId, Car>,
    names: HashMap<PlayerId, String>,

    previous: [Option<Pose>; MAX_CARS],
    current: [Option<Pose>; MAX_CARS],

    player_order: Vec<PlayerId>,
    finish_ticks: [Option<u32>; MAX_CARS],
    following: PlayerId,

    acc: f32,
    render_alpha: f32,
    speed: f32,
    paused: bool,

    clock: f32,
    loading_fade: f32,

    last_tier: u8,
    last_status: Status,
}

pub enum Drawable {
    Car(PlayerId),
    Prop,
}

impl ReplayScene {
    pub fn new(ctx: &mut Context, data: &[u8]) -> Self {
        let replay = Replay::decode(data).expect("invalid replay data");

        let json_path = format!("{}.json", replay.level);
        let png_path = format!("{}.png", replay.level);

        let level = Level::load(&json_path).expect("failed to load replay level");
        let sim_map = grid::Map::from_level(&level);
        let map = VisualMap::load(ctx, &png_path, &json_path, 1);

        let mut names = HashMap::new();
        let mut player_order = Vec::new();
        let mut finish_ticks = [None; MAX_CARS];

        for player in &replay.players {
            let slot = player.slot as usize;
            if slot >= MAX_CARS {
                continue;
            }

            let id = player.slot as PlayerId;

            names.insert(id, player.name.clone());
            player_order.push(id);
            finish_ticks[slot] = player.finish_tick;
        }

        let playback = ReplayPlayback::new(replay).expect("invalid replay simulation");

        let previous = std::array::from_fn(|slot| Self::read_pose(&playback, slot));
        let current = previous;

        let mut cars = HashMap::new();

        for (slot, pose) in current.iter().enumerate() {
            let Some(pose) = pose else {
                continue;
            };

            let id = slot as PlayerId;

            let mut car = Car::new(
                ctx,
                Vector2 {
                    x: pose.pos.x,
                    y: pose.pos.y,
                },
                pose.rot.to_degrees(),
                0.0,
            );

            apply_pose(&mut car, 0.0, pose);
            cars.insert(id, car);
        }

        let shadow_canvas = ctx.rl.load_render_texture(ctx.thread, 320, 180).unwrap();

        let fx = Fx::new(ctx, Vector2::zero(), map.size);

        let following = player_order.first().copied().unwrap_or(0 as PlayerId);

        Self {
            playback,
            level,
            sim_map,
            map,
            camera: Camera::default(),
            shadow_canvas,
            fx,
            cars,
            names,
            previous,
            current,
            player_order,
            finish_ticks,
            following,
            acc: 0.0,
            render_alpha: 0.0,
            speed: 1.0,
            paused: false,
            clock: 0.0,
            loading_fade: 1.0,
            last_tier: 0,
            last_status: Status::Normal,
        }
    }

    fn read_pose(playback: &ReplayPlayback, slot: usize) -> Option<Pose> {
        let c = playback.world().cars[slot];

        if !c.active {
            return None;
        }

        Some(Pose {
            pos: c.pos,
            rot: c.rot,
            z: c.z,
            status: c.status,
            invuln: c.invuln,
            drift_dir: c.drift_dir,
            tier: c.tier(),
            charge: c.charge,
            boost: c.boost,
            boost_lvl: c.boost_lvl,
            heat: c.heat,
            over: c.over,
            perfect: c.perfect,
            draft: false, // todo
            scrape: c.wall_hit,
        })
    }

    fn capture_poses(&mut self) {
        self.current = std::array::from_fn(|slot| Self::read_pose(&self.playback, slot));
    }

    fn sync_cars(&mut self, ctx: &mut Context, now: f32) {
        for slot in 0..MAX_CARS {
            let id = slot as PlayerId;

            let Some(pose) = self.current[slot] else {
                self.cars.remove(&id);
                continue;
            };

            let car = self.cars.entry(id).or_insert_with(|| {
                Car::new(
                    ctx,
                    Vector2 {
                        x: pose.pos.x,
                        y: pose.pos.y,
                    },
                    pose.rot.to_degrees(),
                    0.0,
                )
            });

            apply_pose(car, now, &pose);
        }
    }

    fn step_once(&mut self, ctx: &mut Context) -> bool {
        self.previous = self.current;

        if !self.playback.step(&self.sim_map) {
            return false;
        }

        self.capture_poses();

        let sim_time = self.playback.tick() as f32 * DT;
        self.sync_cars(ctx, sim_time);

        let map = &self.map;
        self.fx
            .update(DT, &self.cars, &self.level, &|p| map.is_water(p));

        true
    }

    fn interpolated_pose(&self, slot: usize) -> Option<Pose> {
        let previous = self.previous[slot];
        let current = self.current[slot];

        match (previous, current) {
            (Some(a), Some(b)) => {
                let t = self.render_alpha;

                Some(Pose {
                    pos: a.pos.lerp(b.pos, t),
                    rot: lerp_angle(a.rot, b.rot, t),
                    z: a.z + (b.z - a.z) * t,

                    status: b.status,
                    invuln: b.invuln,
                    drift_dir: b.drift_dir,
                    tier: b.tier,
                    charge: b.charge,
                    boost: b.boost,
                    boost_lvl: b.boost_lvl,
                    heat: b.heat,
                    over: b.over,
                    perfect: b.perfect,
                    draft: b.draft,
                    scrape: b.scrape,
                })
            }
            (None, Some(current)) => Some(current),
            (_, None) => None,
        }
    }

    fn update_follow_target(&mut self) {
        let tick = self.playback.tick();
        let slot = self.following as usize;

        if slot < MAX_CARS && self.finish_ticks[slot].map_or(false, |finish| tick >= finish) {
            if let Some(next) = self.player_order.iter().copied().find(|id| {
                let slot = *id as usize;

                slot < MAX_CARS && self.finish_ticks[slot].map_or(true, |finish| tick < finish)
            }) {
                self.following = next;
            }
        }

        if !self.cars.contains_key(&self.following) {
            if let Some(id) = self
                .player_order
                .iter()
                .copied()
                .find(|id| self.cars.contains_key(id))
            {
                self.following = id;
            }
        }
    }

    fn update_camera(&mut self, dt: f32) {
        let slot = self.following as usize;

        let pose = if slot < MAX_CARS {
            self.interpolated_pose(slot)
        } else {
            None
        };

        let Some(pose) = pose else {
            return;
        };

        self.camera.set_boost(pose.boost);

        if pose.tier > self.last_tier {
            self.camera.add_shake(0.25);
        }

        if pose.status == Status::Spun && self.last_status != Status::Spun {
            self.camera.add_shake(0.8);
        }

        self.last_tier = pose.tier;
        self.last_status = pose.status;

        self.camera.update(
            Vector2 {
                x: pose.pos.x,
                y: pose.pos.y,
            },
            pose.rot.to_degrees(),
            dt,
        );
    }

    fn prepare_render_cars(&mut self) {
        for slot in 0..MAX_CARS {
            let id = slot as PlayerId;

            let Some(pose) = self.interpolated_pose(slot) else {
                continue;
            };

            if let Some(car) = self.cars.get_mut(&id) {
                apply_pose(car, self.clock, &pose);
            }
        }
    }
}

impl Scene for ReplayScene {
    fn update(&mut self, ctx: &mut Context, dt: f32, _alpha: f32) -> Transition {
        if ctx.rl.is_key_pressed(KeyboardKey::KEY_ESCAPE) {
            return Transition::Switch(Box::new(MenuScene::new("", ctx)));
        }

        if ctx.rl.is_key_pressed(KeyboardKey::KEY_SPACE) {
            self.paused = !self.paused;
        }

        if ctx.rl.is_key_pressed(KeyboardKey::KEY_UP) {
            self.speed = (self.speed * 2.0).min(8.0);
        }

        if ctx.rl.is_key_pressed(KeyboardKey::KEY_DOWN) {
            self.speed = (self.speed / 2.0).max(0.25);
        }

        if !self.paused && !self.playback.finished() {
            self.acc += dt.min(0.25) * self.speed;

            while self.acc >= DT && !self.playback.finished() {
                self.acc -= DT;

                if !self.step_once(ctx) {
                    self.acc = 0.0;
                    break;
                }
            }
        }

        if self.playback.finished() {
            self.acc = 0.0;
            self.render_alpha = 1.0;
        } else {
            self.render_alpha = (self.acc / DT).clamp(0.0, 1.0);
        }

        let tick = self.playback.tick();
        self.clock = if tick == 0 {
            0.0
        } else {
            (tick as f32 - 1.0 + self.render_alpha) * DT
        };

        self.sync_cars(ctx, tick as f32 * DT);

        self.update_follow_target();
        self.update_camera(dt);

        if !self.cars.is_empty() {
            self.loading_fade = (self.loading_fade - dt / 0.5).max(0.0);
        }

        Transition::None
    }

    fn draw(&mut self, ctx: &mut Context) {
        self.fx.flush(ctx);

        self.prepare_render_cars();

        {
            let mut tm = ctx
                .rl
                .begin_texture_mode(&ctx.thread, &mut self.shadow_canvas);

            tm.clear_background(Color::BLANK);
            self.map.draw_shadows(ctx.assets, &mut tm, &self.camera);

            for car in self.cars.values() {
                car.draw_shadow(ctx.assets, &mut tm, &self.camera);
            }
        }

        let mut tm = ctx.rl.begin_texture_mode(ctx.thread, ctx.canvas);
        tm.clear_background(Color::BLACK);

        self.map.draw_layers(ctx.assets, &mut tm, &self.camera);
        self.fx.draw_marks(&mut tm, &self.camera);

        tm.draw_texture_rec(
            self.shadow_canvas.texture(),
            Rectangle::new(0.0, 0.0, 320.0, -180.0),
            Vector2::zero(),
            Color::new(255, 255, 255, 140),
        );

        self.fx.draw_trails(&mut tm, &self.camera);

        let map = &self.map;
        self.fx
            .draw_ripples(&mut tm, &self.camera, &|p| map.is_water(p));

        let mut drawables: Vec<(f32, usize, Drawable)> = Vec::new();
        let bounds = self.camera.visible_bounds(32.0);

        for (i, (id, car)) in self.cars.iter().enumerate() {
            let (pos, _) = self.camera.apply(car.p, car.a);
            drawables.push((pos.y, i, Drawable::Car(*id)));
        }

        for (i, prop) in self.map.props.iter().enumerate() {
            if !VisualMap::visible(prop, bounds) {
                continue;
            }

            let (pos, _) = self.camera.apply(prop.pos, prop.a);
            drawables.push((pos.y, MAX_CARS + i, Drawable::Prop));
        }

        drawables.sort_by(|a, b| {
            a.0.partial_cmp(&b.0)
                .unwrap_or(Ordering::Equal)
                .then(a.1.cmp(&b.1))
        });

        for (_, i, drawable) in drawables {
            match drawable {
                Drawable::Car(id) => {
                    let Some(car) = self.cars.get(&id) else {
                        continue;
                    };

                    let (map, camera) = (&mut self.map, &self.camera);

                    car.draw(
                        ctx.assets,
                        &mut tm,
                        camera,
                        self.clock,
                        |tm, pos, depth| {
                            map.draw_water_overlay(ctx.assets, tm, camera, pos, depth);
                        },
                        false,
                    );
                }
                Drawable::Prop => {
                    self.map
                        .draw_prop(ctx.assets, &mut tm, i - MAX_CARS, &self.camera);
                }
            }
        }

        for (id, car) in &self.cars {
            let Some(name) = self.names.get(id) else {
                continue;
            };

            let (pos, _) = self.camera.apply(car.p, car.a);
            let width = tm.measure_text(name, 8);

            tm.draw_text(
                name,
                pos.x as i32 - width / 2,
                pos.y as i32 - 20 - car.z as i32,
                8,
                Color::WHITE,
            );
        }

        self.fx.draw_flames(&mut tm, &self.camera);
        self.fx.draw_bits(&mut tm, &self.camera);
        self.fx.draw_puffs(&mut tm, &self.camera);

        let map = &self.map;
        self.fx
            .draw_droplets(&mut tm, &self.camera, &|p| map.is_water(p));

        let state = if self.playback.finished() {
            "FINISHED".to_string()
        } else if self.paused {
            "PAUSED".to_string()
        } else {
            format!("PLAYING  {:.2}x", self.speed)
        };

        tm.draw_text(&state, 12, 10, 10, Color::BLACK);
        tm.draw_text(&state, 11, 9, 10, Color::WHITE);

        let time = format_time(self.clock);
        tm.draw_text(&time, 12, 23, 10, Color::BLACK);
        tm.draw_text(&time, 11, 22, 10, Color::WHITE);

        let help = "SPACE: pause   UP/DOWN: speed   ESC: menu";
        let width = tm.measure_text(help, 8);
        tm.draw_text(help, 160 - width / 2, 168, 8, Color::WHITE);

        if self.loading_fade > 0.0 {
            tm.draw_rectangle(
                0,
                0,
                320,
                180,
                Color::new(0, 0, 0, (255.0 * self.loading_fade.clamp(0.0, 1.0)) as u8),
            );
        }
    }

    fn blocks_draw(&self) -> bool {
        true
    }

    fn blocks_update(&self) -> bool {
        true
    }
}

fn lerp_angle(a: f32, b: f32, t: f32) -> f32 {
    use std::f32::consts::{PI, TAU};

    let delta = (b - a + PI).rem_euclid(TAU) - PI;
    a + delta * t
}
