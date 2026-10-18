use std::collections::{HashMap, HashSet, VecDeque};

#[cfg(feature = "debug-renderer")]
use raylib::drawing::RaylibTextureMode;
use raylib::{
    RaylibHandle,
    drawing::{RaylibDraw, RaylibTextureMode, RaylibTextureModeExt},
    ffi::{Color, KeyboardKey, Vector2},
    math::Rectangle,
    texture::{RaylibRenderTexture2D, RenderTexture2D},
};
#[cfg(feature = "debug-renderer")]
use wrum_core::physics::{DebugColor, Vector};
use wrum_core::{
    PlayerId, TICK_DT,
    level::Level,
    physics::{CarState, Input, PhysicsWorld, Status, Vec2},
    race::TOTAL_LAPS,
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

#[derive(Clone, Copy)]
enum StartPhase {
    Waiting,
    Countdown { go_time: f32 },
    Racing,
}

#[allow(unused)]
struct Pose {
    pos: Vec2,
    rot: f32,
    z: f32,
    vz: f32,
    speed: f32,
    status: Status,
    invuln: u8,
}

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

    order: Vec<PlayerId>,
    last_order_tick: u32,
    lap: u32,

    code: String,
    private: bool,

    clock: f32,

    phase: StartPhase,
    host: PlayerId,
    ready: HashSet<PlayerId>,
    go_at: f32,
    snap_clock_at: f32,
}

impl GameScene {
    pub fn new(
        ctx: &mut ctx::Context,
        my_id: PlayerId,
        spawn: &CarState,
        existing: &Vec<(PlayerId, CarState)>,
        code: &str,
        private: bool,
    ) -> Self {
        let level = Level::load("assets/maps/untitled.json").unwrap();
        let map = Map::load(ctx, "assets/maps/untitled.json");

        let mut physics = PhysicsWorld::default();
        physics.authoritative = false;
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
            order: Vec::new(),
            last_order_tick: 0,
            lap: 1,
            code: code.into(),
            private,
            clock: 0.,
            phase: StartPhase::Waiting,
            host: 0,
            ready: HashSet::new(),
            go_at: 0.,
            snap_clock_at: 0.,
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
        if !matches!(self.phase, StartPhase::Racing) {
            input.throttle = 0;
            input.steer = 0;
            input.drift = false;
        }
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
                if let Some(p) = sample(q, t) {
                    self.physics.set_remote(*id, p.pos, p.rot);
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
        self.snap_clock_at = self.clock;
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

    fn my_place(&self) -> Option<(usize, usize)> {
        let i = self.order.iter().position(|&id| id == self.my_id)?;
        Some((i + 1, self.order.len()))
    }

    fn server_now(&self) -> f32 {
        self.latest_server_time + (self.clock - self.snap_clock_at)
    }
}

impl Scene for GameScene {
    fn fixed_update(&mut self, ctx: &mut ctx::Context) -> super::Transition {
        self.sim_tick(ctx);
        super::Transition::None
    }
    fn update(&mut self, ctx: &mut ctx::Context, dt: f32, alpha: f32) -> super::Transition {
        if let StartPhase::Countdown { go_time } = self.phase {
            if self.server_now() >= go_time {
                self.phase = StartPhase::Racing;
                self.go_at = self.clock;
            }
        }

        if matches!(self.phase, StartPhase::Waiting)
            && ctx.rl.is_key_pressed(KeyboardKey::KEY_ENTER)
        {
            let msg = if self.private {
                (self.host == self.my_id).then_some(ClientMsg::StartRace)
            } else {
                Some(ClientMsg::Ready(!self.ready.contains(&self.my_id)))
            };
            if let Some(m) = msg {
                let _ = ctx.net.send(Channel::Reliable, &m);
            }
        }

        self.clock += dt;
        let now = self.clock;

        let k = (-ERR_DECAY * dt).exp();
        self.err_pos *= k;
        self.err_rot *= k;

        let pos = self.prev.pos.lerp(self.curr.pos, alpha) + self.err_pos;
        let rot = self.prev.rot + wrap_angle(self.curr.rot - self.prev.rot) * alpha + self.err_rot;
        let z = self.prev.z + (self.curr.z - self.prev.z) * alpha;
        let me = Pose {
            pos,
            rot,
            z,
            vz: self.curr.vz,
            speed: self.curr.vel.length(),
            status: self.curr.status,
            invuln: self.curr.invuln,
        };
        if let Some(car) = self.cars.get_mut(&self.my_id) {
            apply_pose(car, now, &me);
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
                if let (Some(p), Some(car)) = (sample(q, self.render_time), self.cars.get_mut(id)) {
                    apply_pose(car, now, &p);
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
                    let (map, cam) = (&mut self.map, &self.camera);
                    car.draw(&mut tm, &self.camera, self.clock, |tm, pos, depth| {
                        map.draw_water_overlay(tm, cam, pos, depth);
                    });
                }
                Drawable::Prop => {
                    self.map.draw_prop(&mut tm, i, &self.camera);
                }
            }
        }

        #[cfg(feature = "debug-renderer")]
        render(&mut tm, &self.camera, &self.physics.debug_backend.lines);

        if let Some((place, _total)) = self.my_place() {
            let suffix = match place {
                1 => "st",
                2 => "nd",
                3 => "rd",
                _ => "th",
            };
            let rank = format!("{place}{suffix}");
            let lap = format!("Lap {}/{}", self.lap, TOTAL_LAPS);
            let w_rank = tm.measure_text(&rank, 20);
            let w_lap = tm.measure_text(&lap, 10);
            tm.draw_text(&rank, 310 - w_rank, 12, 20, Color::BLACK);
            tm.draw_text(&rank, 308 - w_rank, 10, 20, Color::WHITE);
            tm.draw_text(&lap, 309 - w_lap, 31, 10, Color::BLACK);
            tm.draw_text(&lap, 308 - w_lap, 30, 10, Color::WHITE);
        }

        match self.phase {
            StartPhase::Waiting => {
                tm.draw_text(&self.code, 12, 12, 20, Color::BLACK);
                tm.draw_text(&self.code, 10, 10, 20, Color::WHITE);
                let hint = if self.private {
                    if self.host == self.my_id {
                        "ENTER: start race".to_string()
                    } else {
                        "Waiting for host...".to_string()
                    }
                } else {
                    let me = self.ready.contains(&self.my_id);
                    format!(
                        "ENTER: {}   ({} ready)",
                        if me { "unready" } else { "ready" },
                        self.ready.len()
                    )
                };
                let w = tm.measure_text(&hint, 10);
                tm.draw_text(&hint, 160 - w / 2, 150, 10, Color::WHITE);
            }
            StartPhase::Countdown { go_time } => {
                let rem = go_time - self.server_now();
                if rem > 0.0 {
                    let n = rem.ceil();
                    let p = 1.0 - (rem - (n - 1.0));
                    big_text(&mut tm, &format!("{}", n as i32), (48.0 - 20.0 * p) as i32);
                }
            }
            StartPhase::Racing => {
                if self.clock - self.go_at < 1.0 {
                    big_text(&mut tm, "GO", 48);
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
            ServerMsg::PlayerLeft { id } => {
                self.physics.remove_remote(*id);
                self.remotes.remove(id);
                self.cars.remove(id);
            }
            ServerMsg::Race {
                id,
                lap,
                next: _,
                finished: _,
                lap_time: _,
            } => {
                if *id == self.my_id {
                    self.lap = *lap;
                }
            }
            ServerMsg::Positions { tick, order } => {
                if *tick > self.last_order_tick {
                    self.last_order_tick = *tick;
                    self.order = order.clone();
                }
            }
            ServerMsg::Countdown { go_tick } => {
                self.phase = StartPhase::Countdown {
                    go_time: *go_tick as f32 * TICK_DT,
                }
            }
            ServerMsg::LobbyState { host, ready } => {
                self.host = *host;
                self.ready = ready.iter().copied().collect();
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

fn big_text(tm: &mut RaylibTextureMode<'_, '_, RaylibHandle>, s: &str, size: i32) {
    let w = tm.measure_text(s, size);
    let (x, y) = (160 - w / 2, 70 - size / 2);
    tm.draw_text(s, x + 2, y + 2, size, Color::BLACK);
    tm.draw_text(s, x, y, size, Color::WHITE);
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
fn sample(q: &VecDeque<(f32, CarState)>, t: f32) -> Option<Pose> {
    let pose = |s: &CarState, pos, rot, z| Pose {
        pos,
        rot,
        z,
        vz: s.vz,
        speed: s.vel.length(),
        status: s.status,
        invuln: s.invuln,
    };
    let (t_first, first) = q.front()?;
    if t <= *t_first {
        return Some(pose(first, first.pos, first.rot, first.z));
    }
    for ((t0, s0), (t1, s1)) in q.iter().zip(q.iter().skip(1)) {
        if t >= *t0 && t <= *t1 {
            let f = (t - t0) / (t1 - t0);
            return Some(pose(
                s0,
                s0.pos.lerp(s1.pos, f),
                s0.rot + wrap_angle(s1.rot - s0.rot) * f,
                s0.z + (s1.z - s0.z) * f,
            ));
        }
    }
    let (_, l) = q.back()?;
    Some(pose(l, l.pos, l.rot, l.z))
}
fn apply_pose(car: &mut Car, now: f32, p: &Pose) {
    car.p = Vector2 {
        x: p.pos.x,
        y: p.pos.y,
    };
    car.a = p.rot.to_degrees();
    car.z = p.z;
    car.invuln = p.invuln;
    if p.status == Status::Drowning && car.status != Status::Drowning {
        car.sunk_at = now;
    }
    car.status = p.status;
}

#[cfg(feature = "debug-renderer")]
fn render(
    tm: &mut RaylibTextureMode<'_, '_, RaylibHandle>,
    camera: &Camera,
    lines: &Vec<(Vector, Vector, DebugColor)>,
) {
    for (s, e, c) in lines {
        let (s, _) = camera.apply(Vector2 { x: s.x, y: s.y }, 0.0);
        let (e, _) = camera.apply(Vector2 { x: e.x, y: e.y }, 0.0);

        tm.draw_line_v(s, e, to_rgba(*c));
    }
}
#[cfg(feature = "debug-renderer")]
pub fn to_rgba([h, s, l, a]: DebugColor) -> Color {
    let (h, s, l) = (h / 360.0, s, l);

    let f = |n: f32| {
        let k = (n + h * 12.0) % 12.0;
        let x = s * l.min(1.0 - l);
        l - x * (-1.0_f32).max((k - 3.0).min(9.0 - k).min(1.0))
    };

    Color::new(
        (f(0.0) * 255.0).round() as u8,
        (f(8.0) * 255.0).round() as u8,
        (f(4.0) * 255.0).round() as u8,
        (a.clamp(0.0, 1.0) * 255.0).round() as u8,
    )
}
