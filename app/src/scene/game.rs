use std::collections::{HashMap, VecDeque};

use raylib::{
    RaylibHandle,
    drawing::{RaylibDraw, RaylibTextureModeExt},
    ffi::{Color, KeyboardKey, Vector2},
    math::Rectangle,
    texture::{RaylibRenderTexture2D, RenderTexture2D},
};
use wrum_core::{
    PlayerId, TICK_DT,
    level::Level,
    physics::{CarState, Input, PhysicsWorld, Vec2},
};
use wrum_net::{
    Channel,
    proto::{ClientMsg, ServerMsg},
};

use crate::{camera::Camera, car::Car, ctx, map::Map, scene::Scene};

const INTERP_DELAY: f32 = 0.1;
const MAX_PENDING: usize = 120;
const ERR_DECAY: f32 = 12.0;
const SNAP_DIST: f32 = 40.0;
const REDUNDANCY: usize = 3;
const GHOST_LEAD: f32 = INTERP_DELAY;

pub struct GameScene {
    camera: Camera,
    cars: HashMap<PlayerId, Car>,
    my_id: PlayerId,
    map: Map,
    shadow_canvas: RenderTexture2D,

    recent: VecDeque<Input>,
    physics: PhysicsWorld,
    next_seq: u32,
    pending: VecDeque<Input>,
    prev: CarState,
    curr: CarState,
    err_pos: Vec2,
    err_rot: f32,
    last_snap_tick: u32,

    remotes: HashMap<PlayerId, VecDeque<(f32, CarState)>>,
    render_time: f32,
    latest_server_time: f32,
    have_clock: bool,
}

impl GameScene {
    pub fn new(
        ctx: &mut ctx::Context,
        my_id: PlayerId,
        spawn: &CarState,
        existing: &Vec<(PlayerId, CarState)>,
    ) -> Self {
        let level = Level::load("assets/maps/untitled.json").unwrap();
        let map = Map::load(ctx, "assets/maps/untitled.json");

        let mut physics = PhysicsWorld::default();
        physics.load_level(&level);
        physics.spawn_car(my_id, spawn.pos, spawn.rot);
        physics.set_state(my_id, spawn);

        let shadow_canvas = ctx.rl.load_render_texture(ctx.thread, 320, 180).unwrap();

        let mut s = Self {
            camera: Camera::default(),
            cars: HashMap::new(),
            my_id,
            map,
            shadow_canvas,
            recent: VecDeque::new(),
            physics,
            next_seq: 0,
            pending: VecDeque::new(),
            prev: *spawn,
            curr: *spawn,
            err_pos: Vec2::ZERO,
            err_rot: 0.0,
            last_snap_tick: 0,
            remotes: HashMap::new(),
            render_time: 0.0,
            latest_server_time: 0.0,
            have_clock: false,
        };
        s.ensure_car(ctx, my_id, spawn);
        for (id, st) in existing {
            s.ensure_car(ctx, *id, st);
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
        // input ---------------------------------------------------
        self.next_seq = self.next_seq.wrapping_add(1);
        let mut input = sample_input(ctx.rl);
        input.seq = self.next_seq;

        self.recent.push_back(input);
        while self.recent.len() > REDUNDANCY {
            self.recent.pop_front();
        }
        let inputs: Vec<Input> = self.recent.iter().copied().collect();

        if let Err(e) = ctx
            .net
            .send(Channel::Unreliable, &ClientMsg::Input { inputs })
        {
            eprintln!("failed to send input: {e:#?}");
        }

        // prediction ----------------------------------------------
        if self.have_clock {
            let t = self.render_time + GHOST_LEAD;
            for (id, q) in &self.remotes {
                if let Some((p, r)) = sample(q, t) {
                    self.physics.set_remote(*id, p, r);
                }
            }
        }

        self.physics.set_input(self.my_id, input);
        self.physics.step();
        self.prev = self.curr;
        if let Some(s) = self.physics.state(self.my_id) {
            self.curr = s;
        }

        self.pending.push_back(input);
        while self.pending.len() > MAX_PENDING {
            self.pending.pop_front();
        }
    }
    fn apply_snapshot(
        &mut self,
        ctx: &mut ctx::Context,
        tick: u32,
        cars: &Vec<(PlayerId, CarState)>,
    ) {
        if tick <= self.last_snap_tick {
            return;
        }
        self.last_snap_tick = tick;

        let t = tick as f32 * TICK_DT;
        self.latest_server_time = self.latest_server_time.max(t);
        if !self.have_clock {
            self.render_time = t - INTERP_DELAY;
            self.have_clock = true;
        }

        for (id, state) in cars {
            if *id == self.my_id {
                self.reconcile(state);
            } else {
                self.ensure_car(ctx, *id, state);
                if !self.remotes.contains_key(id) {
                    self.physics.spawn_remote(*id, state);
                }
                let q = self.remotes.entry(*id).or_default();
                q.push_back((t, *state));
                while q.len() > 32 {
                    q.pop_front();
                }
            }
        }
    }
    fn reconcile(&mut self, server: &CarState) {
        let old = self.curr;

        while self
            .pending
            .front()
            .is_some_and(|i| (i.seq.wrapping_sub(server.ack) as i32) <= 0)
        {
            self.pending.pop_front();
        }

        self.physics.set_state(self.my_id, server);
        for input in &self.pending {
            self.physics.set_input(self.my_id, *input);
            self.physics.step();
        }
        let Some(new) = self.physics.state(self.my_id) else {
            return;
        };

        let dp = new.pos - old.pos;
        let dr = wrap_angle(new.rot - old.rot);

        if dp.length() > SNAP_DIST || dr.abs() > 1.0 {
            self.prev = new;
            self.err_pos = Vec2::ZERO;
            self.err_rot = 0.0;
        } else {
            self.err_pos -= dp;
            self.err_rot -= dr;
            self.prev.pos += dp;
            self.prev.rot += dr;
        }
        self.curr = new;
    }
}

impl Scene for GameScene {
    fn fixed_update(&mut self, ctx: &mut ctx::Context) -> super::Transition {
        self.sim_tick(ctx);
        super::Transition::None
    }
    fn update(&mut self, _ctx: &mut ctx::Context, dt: f32, alpha: f32) -> super::Transition {
        let k = (-ERR_DECAY * dt).exp();
        self.err_pos *= k;
        self.err_rot *= k;

        let pos = self.prev.pos.lerp(self.curr.pos, alpha) + self.err_pos;
        let rot = self.prev.rot + wrap_angle(self.curr.rot - self.prev.rot) * alpha + self.err_rot;
        if let Some(car) = self.cars.get_mut(&self.my_id) {
            car.p = Vector2 { x: pos.x, y: pos.y };
            car.a = rot.to_degrees();
        }

        if self.have_clock {
            self.render_time += dt;
            let target = self.latest_server_time - INTERP_DELAY;
            let diff = target - self.render_time;
            if diff.abs() > 0.25 {
                self.render_time = target;
            } else {
                self.render_time += diff * (1.0 - (-2.0 * dt).exp());
            }

            for (id, q) in self.remotes.iter_mut() {
                while q.len() > 2 && q[1].0 <= self.render_time {
                    q.pop_front();
                }
                if let (Some((p, r)), Some(car)) =
                    (sample(q, self.render_time), self.cars.get_mut(id))
                {
                    car.p = Vector2 { x: p.x, y: p.y };
                    car.a = r.to_degrees();
                }
            }
        }

        if let Some(me) = self.cars.get(&self.my_id) {
            self.camera.update(me.p, me.a, dt);
        }
        super::Transition::None
    }
    fn draw(&mut self, ctx: &mut ctx::Context) {
        {
            let mut tm = ctx
                .rl
                .begin_texture_mode(&ctx.thread, &mut self.shadow_canvas);
            tm.clear_background(Color::BLANK);
            self.map.draw_shadows(&mut tm, &self.camera);

            for (_, car) in &self.cars {
                car.draw_shadow(&mut tm, &self.camera);
            }
        }

        let mut tm = ctx.rl.begin_texture_mode(ctx.thread, ctx.canvas);
        tm.clear_background(Color::BLACK);

        self.map.draw_layers(&mut tm, &self.camera);
        tm.draw_texture_rec(
            self.shadow_canvas.texture(),
            Rectangle::new(0., 0., 320., -180.),
            Vector2::zero(),
            Color::new(255, 255, 255, 180),
        );

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
                    car.draw(&mut tm, &self.camera);
                }
                Drawable::Prop => {
                    self.map.draw_prop(&mut tm, i, &self.camera);
                }
            }
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

pub enum Drawable {
    Car(PlayerId),
    Prop,
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
fn wrap_angle(a: f32) -> f32 {
    use std::f32::consts::{PI, TAU};
    (a + PI).rem_euclid(TAU) - PI
}
fn sample(q: &VecDeque<(f32, CarState)>, t: f32) -> Option<(Vec2, f32)> {
    let (t_first, first) = q.front()?;
    if t <= *t_first {
        return Some((first.pos, first.rot));
    }

    for (a, b) in q.iter().zip(q.iter().skip(1)) {
        let (t0, s0) = a;
        let (t1, s1) = b;
        if t >= *t0 && t <= *t1 {
            let f = (t - t0) / (t1 - t0);
            return Some((
                s0.pos.lerp(s1.pos, f),
                s0.rot + wrap_angle(s1.rot - s0.rot) * f,
            ));
        }
    }
    let (_, last) = q.back()?;
    Some((last.pos, last.rot))
}
