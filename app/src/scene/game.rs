use std::collections::{HashMap, VecDeque};

use raylib::{
    RaylibHandle,
    color::Color,
    drawing::{RaylibDraw, RaylibTextureMode, RaylibTextureModeExt},
    ffi::KeyboardKey,
    math::{Rectangle, Vector2, lerp},
    texture::{RaylibRenderTexture2D, RenderTexture2D},
};
use shared::{
    coll::{Obb, Vec2},
    interp::{INTERP_DELAY_TICKS, INTERP_SMOOTH, INTERP_SNAP_TICKS, SnapshotBuffer, lerp_angle},
    net::{
        Channel, ClientMessage,
        proto::{INPUT_REDUNDANCY, Snapshot, Welcome},
    },
    sim::{CAR_HALFS, CAR_RESTITUTION, CarState, Input, TICK_DT},
};

use crate::{
    camera::Camera,
    entity::car::Car,
    scene::{Scene, menu::MenuScene},
};

const PENDING_CAP: usize = 64;
const RECON_SNAP_DIST: f32 = 100.0;
const RECON_DECAY: f32 = 12.0;

use std::f32::consts::PI;

fn angle_diff(from: f32, to: f32) -> f32 {
    let mut d = to - from;
    while d > PI {
        d -= 2.0 * PI;
    }
    while d < -PI {
        d += 2.0 * PI;
    }
    d
}

#[derive(Clone)]
struct InputRing {
    items: VecDeque<Input>,
}
impl InputRing {
    fn new() -> Self {
        Self {
            items: VecDeque::with_capacity(PENDING_CAP),
        }
    }
    fn push(&mut self, input: Input) {
        if self.items.len() == PENDING_CAP {
            self.items.pop_front();
        }
        self.items.push_back(input);
    }
    fn discard_upto(&mut self, seq: u32) {
        while self.items.front().is_some_and(|x| x.seq <= seq) {
            self.items.pop_front();
        }
    }
    fn recent(&self, n: usize) -> Vec<Input> {
        let n = n.min(self.items.len());
        self.items
            .iter()
            .skip(self.items.len() - n)
            .copied()
            .collect()
    }
}

pub struct GameScene {
    shadow_c: RenderTexture2D,
    camera: Camera,
    // map: Map,
    // level: Level
    //
    cars: HashMap<u8, Car>,

    my_id: u8,
    code: String,

    last_server_tick: u32,
    pending: InputRing,
    prev_sim: CarState,

    err_p: Vector2,
    err_a: f32,

    snapbuf: SnapshotBuffer,
    render_tick: f32,
    acc: f32,

    pending_spawn: CarState,
}

impl GameScene {
    pub fn new(ctx: &mut crate::ctx::Context, msg: &Welcome) -> Self {
        let shadow_c = ctx.rl.load_render_texture(&ctx.thread, 320, 180).unwrap();
        Self {
            shadow_c,
            camera: Camera::default(),
            cars: HashMap::new(),
            my_id: msg.your_id,
            code: msg.lobby_code.clone(),
            last_server_tick: 0,
            pending: InputRing::new(),
            prev_sim: CarState::default(),
            err_p: Vector2::zero(),
            err_a: 0.,
            snapbuf: SnapshotBuffer::new(),
            render_tick: 0.,
            acc: 0.,
            pending_spawn: msg.spawn,
        }
    }

    fn ensure_car(&mut self, ctx: &mut crate::ctx::Context, id: u8, spawn: CarState) {
        self.cars.entry(id).or_insert_with(|| {
            let mut car = Car::new(
                ctx,
                Vector2::new(spawn.pos.x, spawn.pos.y),
                spawn.a.to_degrees(),
                spawn.t.to_degrees(),
            );
            car.sim = spawn;
            car
        });
    }

    fn sim_tick(&mut self, ctx: &mut crate::ctx::Context) {
        let mut input = sample_input(&ctx.rl);
        input.seq = ctx.net.next_seq;
        ctx.net.next_seq += 1;

        // sim

        let ghosts: Vec<Obb> = self
            .cars
            .iter()
            .filter(|(id, _)| **id != self.my_id)
            .map(|(_, car)| Obb {
                pos: Vec2 {
                    x: car.p.x,
                    y: car.p.y,
                },
                halfs: CAR_HALFS,
                a: car.a.to_radians(),
            })
            .collect();
        let me = self.cars.get_mut(&self.my_id).unwrap();

        self.prev_sim = me.sim;
        me.sim.step(&input);
        // level_resolve_car(g->level, &me->sim);

        for ghost in ghosts {
            me.sim.resolve_vs_obstacle(ghost, CAR_RESTITUTION);
        }

        // input

        self.pending.push(input);
        let msg = ClientMessage::Input {
            inputs: self.pending.recent(INPUT_REDUNDANCY),
        };

        if let Err(e) = ctx.net.send(Channel::Unreliable, &msg) {
            eprintln!("failed to send input: {e:?}");
        }
    }
    fn apply_snapshot(&mut self, ctx: &mut crate::ctx::Context, snap: &mut Snapshot) {
        self.last_server_tick = snap.server_tick;

        for (id, state) in &mut snap.ents {
            self.ensure_car(ctx, *id, state.clone());

            if *id != self.my_id {
                continue;
            }

            self.pending.discard_upto(snap.ack_seq);

            let before = self.cars.get(&self.my_id).unwrap().sim;
            let mut sim = *state;

            let ghosts: Vec<Obb> = self
                .cars
                .iter()
                .filter(|(id, _)| **id != self.my_id)
                .map(|(_, car)| Obb {
                    pos: Vec2 {
                        x: car.p.x,
                        y: car.p.y,
                    },
                    halfs: CAR_HALFS,
                    a: car.a.to_radians(),
                })
                .collect();

            for input in &self.pending.items {
                sim.step(input);
                // level_resolve_car(&self.level, &mut sim);
                for ghost in &ghosts {
                    sim.resolve_vs_obstacle(*ghost, CAR_RESTITUTION);
                }
            }

            self.cars.get_mut(&self.my_id).unwrap().sim = sim;

            let dx = before.pos.x - sim.pos.x;
            let dy = before.pos.y - sim.pos.y;

            let mut da = angle_diff(sim.a, before.a);

            let mut dx = dx;
            let mut dy = dy;

            if dx * dx + dy * dy > RECON_SNAP_DIST * RECON_SNAP_DIST {
                dx = 0.0;
                dy = 0.0;
            }

            if da.abs() > 1.57 {
                da = 0.0;
            }

            self.err_p.x += dx;
            self.err_p.y += dy;
            self.err_a += da;
        }

        self.snapbuf.push(snap);
    }

    fn update_render_tick(&mut self, dt: f32) {
        let Some(newest) = self.snapbuf.latest() else {
            return;
        };

        self.render_tick += dt / TICK_DT;

        let target = newest.tick as f32 - INTERP_DELAY_TICKS;
        let diff = target - self.render_tick;

        if diff.abs() > INTERP_SNAP_TICKS {
            self.render_tick = target;
        } else {
            self.render_tick += diff * INTERP_SMOOTH;
        }
    }
    fn interp_others(&mut self) {
        let Some((a, c, t)) = self.snapbuf.find_bracket(self.render_tick) else {
            return;
        };

        for (id, car) in self.cars.iter_mut() {
            if *id == self.my_id {
                continue;
            }

            let sa = a.find_entity(*id);
            let sc = c.find_entity(*id);

            let mut r = CarState::default();
            if let Some(sa) = sa
                && let Some(sc) = sc
            {
                r.pos.x = lerp(sa.pos.x, sc.pos.x, t);
                r.pos.y = lerp(sa.pos.y, sc.pos.y, t);
                r.a = lerp_angle(sa.a, sc.a, t);
                r.t = lerp_angle(sa.t, sc.t, t);
            } else if let Some(sc) = sc {
                r = *sc;
            } else if let Some(sa) = sa {
                r = *sa;
            } else {
                continue;
            }

            car.p = Vector2 {
                x: r.pos.x,
                y: r.pos.y,
            };
            car.a = r.a.to_degrees();
            car.t = r.t.to_degrees();
        }
    }
    fn update_own_render(&mut self, dt: f32) {
        let me = self.cars.get_mut(&self.my_id).unwrap();

        let alpha = (self.acc / TICK_DT).clamp(0., 1.);
        let ix = lerp(self.prev_sim.pos.x, me.sim.pos.x, alpha);
        let iy = lerp(self.prev_sim.pos.y, me.sim.pos.y, alpha);
        let ia = lerp_angle(self.prev_sim.a, me.sim.a, alpha);
        let it = lerp(self.prev_sim.t, me.sim.t, alpha);

        let decay = (-RECON_DECAY * dt).exp();
        self.err_p.x *= decay;
        self.err_p.y *= decay;
        self.err_a *= decay;

        me.p.x = ix + self.err_p.x;
        me.p.y = iy + self.err_p.y;
        me.a = (ia + self.err_a).to_degrees();
        me.t = it.to_degrees();
    }
}

impl Scene for GameScene {
    fn update(&mut self, ctx: &mut crate::ctx::Context, dt: f32) -> super::Transition {
        self.acc += dt;
        if self.acc > 0.25 {
            self.acc = 0.25;
        }
        while self.acc >= TICK_DT {
            self.acc -= TICK_DT;
            self.sim_tick(ctx);
        }
        self.update_own_render(dt);
        self.update_render_tick(dt);
        self.interp_others();

        let me = self.cars.get(&self.my_id).unwrap();
        self.camera.update(me.p, me.a, dt);

        // todo:
        // if (IsKeyPressed(KEY_ESCAPE)) scene_push(make_pause_scene());

        super::Transition::None
    }
    fn draw(&mut self, ctx: &mut crate::ctx::Context) {
        {
            let mut tm = ctx.rl.begin_texture_mode(&ctx.thread, &mut self.shadow_c);
            tm.clear_background(Color::BLANK);
            // map_draw_shadows(g->map, &g->camera);

            for (_, car) in &self.cars {
                car.draw_shadow(&mut tm, &self.camera);
            }
        }

        let mut tm = ctx.rl.begin_texture_mode(&ctx.thread, &mut ctx.canvas);
        tm.clear_background(Color::BLACK);

        // map_draw_layers(g->map, &g->camera);

        tm.draw_texture_rec(
            self.shadow_c.texture(),
            Rectangle::new(0., 0., 320., -180.),
            Vector2::zero(),
            Color::new(255, 255, 255, 180),
        );

        // (y, index, drawable)
        let mut drawables: Vec<(f32, u8, &dyn Drawable)> = Vec::new();
        for (id, car) in &self.cars {
            let (pos, _) = self.camera.apply(car.p, car.a);
            drawables.push((pos.y, *id, car));
        }

        drawables.sort_by(|a, b| {
            a.0.partial_cmp(&b.0)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(a.1.cmp(&b.1))
        });

        for (_, _, d) in drawables {
            d.draw_y_sort(&mut tm, &self.camera);
        }

        if self.code.len() != 0 {
            tm.draw_text(&self.code, 20, 10, 10, Color::WHITE);
        }
    }

    fn enter(&mut self, ctx: &mut crate::ctx::Context) {
        self.ensure_car(ctx, self.my_id, self.pending_spawn);
    }

    fn on_message(
        &mut self,
        ctx: &mut crate::ctx::Context,
        msg: shared::net::ServerMessage,
    ) -> super::Transition {
        match msg {
            shared::net::ServerMessage::Snapshot(mut snap) => {
                self.apply_snapshot(ctx, &mut snap);
            }
            _ => {}
        }

        super::Transition::None
    }
    fn on_disconnect(
        &mut self,
        _ctx: &mut crate::ctx::Context,
        _reason: shared::net::DisconnectReason,
    ) -> super::Transition {
        super::Transition::Switch(Box::new(MenuScene::new("disconnected from server")))
    }

    fn blocks_update(&self) -> bool {
        true
    }
    fn blocks_draw(&self) -> bool {
        true
    }
}

pub trait Drawable {
    fn draw_y_sort(&self, tm: &mut RaylibTextureMode<'_, RaylibHandle>, camera: &Camera);
}
impl Drawable for Car {
    fn draw_y_sort(&self, tm: &mut RaylibTextureMode<'_, RaylibHandle>, camera: &Camera) {
        self.draw(tm, camera);
    }
}

fn sample_input(rl: &RaylibHandle) -> Input {
    Input {
        seq: 0,
        steer: rl.is_key_down(KeyboardKey::KEY_D) as i8 - rl.is_key_down(KeyboardKey::KEY_A) as i8,
        thr: rl.is_key_down(KeyboardKey::KEY_W) as i8 - rl.is_key_down(KeyboardKey::KEY_S) as i8,
    }
}
