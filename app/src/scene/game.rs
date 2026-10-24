use std::collections::{HashMap, HashSet};

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
    PlayerId,
    level::Level,
    race::TOTAL_LAPS,
    sim::{DT, Input, MAX_CARS, Status, V2, grid},
};
use wrum_net::proto::{ClientMsg, PlayerResult, ServerMsg};

use crate::{
    camera::Camera,
    car::Car,
    ctx,
    fx::Fx,
    map::Map,
    scene::{Scene, Transition},
    sim::NetSim,
};

#[derive(Clone, Copy)]
enum StartPhase {
    Waiting,
    Countdown { go_tick: u32 },
    Racing,
}

#[allow(unused)]
struct Pose {
    pos: V2,
    rot: f32,
    z: f32,
    vz: f32,
    speed: f32,
    status: Status,
    invuln: u8,
}

pub struct GameScene {
    camera: Camera,
    last_camera_pos: (Vector2, f32),
    following: PlayerId,

    cars: HashMap<PlayerId, Car>,
    my_id: PlayerId,
    map: Map,
    shadow_canvas: RenderTexture2D,

    order: Vec<PlayerId>,
    last_order_tick: u32,
    lap: u32,

    sim: NetSim,

    code: String,
    private: bool,

    clock: f32,

    phase: StartPhase,
    host: PlayerId,
    ready: HashSet<PlayerId>,
    go_at: f32,

    finished: bool,
    remote_finished: HashSet<PlayerId>,
    game_finished: bool,

    results: Option<(Vec<PlayerId>, Vec<PlayerResult>)>,

    level: Level,
    fx: Fx,
}

impl GameScene {
    pub fn new(
        ctx: &mut ctx::Context,
        my_id: PlayerId,
        _players: &Vec<(PlayerId, String)>,
        code: &str,
        private: bool,
    ) -> Self {
        let level = Level::load("assets/maps/untitled.json").unwrap();
        let map = Map::load(ctx, "assets/maps/untitled.json");

        let map_grid = grid::Map::from_level(&level);
        let sim = NetSim::new(map_grid, my_id as usize);

        let shadow_canvas = ctx.rl.load_render_texture(ctx.thread, 320, 180).unwrap();

        let fx = Fx::new(ctx, Vector2::zero(), map.size);

        let s = Self {
            camera: Camera::default(),
            last_camera_pos: (Vector2::ZERO, 0.),
            following: my_id,
            cars: HashMap::new(),
            my_id,
            map,
            shadow_canvas,
            sim,
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
            finished: false,
            remote_finished: HashSet::new(),
            game_finished: false,
            results: None,
            fx,
            level,
        };
        // for (id, st) in players {
        //     s.ensure_car(ctx, *id, st.pos, st.rot);
        // }
        s
    }

    fn ensure_car(&mut self, ctx: &mut ctx::Context, id: PlayerId, pos: V2, rot_rad: f32) {
        self.cars.entry(id).or_insert_with(|| {
            Car::new(
                ctx,
                Vector2 { x: pos.x, y: pos.y },
                rot_rad.to_degrees(),
                0.,
                0.,
            )
        });
    }

    fn my_place(&self) -> Option<(usize, usize)> {
        let i = self.order.iter().position(|&id| id == self.my_id)?;
        Some((i + 1, self.order.len()))
    }
    fn next_unfinished(&self) -> Option<PlayerId> {
        self.order
            .iter()
            .copied()
            .find(|id| !self.remote_finished.contains(id) && (*id != self.my_id || !self.finished))
    }

    fn reset_race(&mut self) {
        self.following = self.my_id;
        self.finished = false;
        self.game_finished = false;
        self.remote_finished.clear();

        self.order.clear();
        self.last_order_tick = 0;
        self.lap = 1;

        self.phase = StartPhase::Waiting;
        self.go_at = 0.0;
        self.last_camera_pos = (Vector2::ZERO, 0.0);

        self.results = None;

        self.sim.go_tick = None;
    }
}

impl Scene for GameScene {
    fn fixed_update(&mut self, _ctx: &mut ctx::Context) -> Transition {
        Transition::None
    }
    fn update(&mut self, ctx: &mut ctx::Context, dt: f32, _alpha: f32) -> super::Transition {
        self.clock += dt;

        if let StartPhase::Countdown { go_tick } = self.phase {
            if (self.sim.tick().wrapping_sub(go_tick) as i32) >= 0 {
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
                let _ = ctx.net.send(&m);
            }
        }

        let input = if self.game_finished {
            Input::default()
        } else {
            sample_input(ctx.rl)
        };
        let mut out = Vec::new();
        self.sim.update(dt.min(0.25), input, &mut out);
        for m in out {
            let _ = ctx.net.send(&m);
        }

        if self.sim.have_world() {
            for i in 0..MAX_CARS {
                let id = i as PlayerId;
                let c = self.sim.w.cars[i];
                if !c.active {
                    self.cars.remove(&id);
                    continue;
                }
                self.ensure_car(ctx, id, c.pos, c.rot);
                let (pos, rot, z) = self.sim.view(i);
                let pose = Pose {
                    pos,
                    rot,
                    z,
                    vz: c.vz,
                    speed: c.speed(),
                    status: c.status,
                    invuln: c.invuln,
                };
                if let Some(car) = self.cars.get_mut(&id) {
                    apply_pose(car, self.clock, &pose);
                }
            }
        }

        self.fx.update(dt, &self.cars, &self.level);

        if self.game_finished {
            self.camera
                .update(self.last_camera_pos.0, self.last_camera_pos.1, dt);
        } else {
            if let Some(c) = self.cars.get(&self.following) {
                self.camera.update(c.p, c.a, dt);
            }
        }

        Transition::None
    }
    fn draw(&mut self, ctx: &mut ctx::Context) {
        self.fx.flush(ctx);
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
        self.fx.draw_marks(&mut tm, &self.camera);
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
        self.fx.draw_puffs(&mut tm, &self.camera);

        #[cfg(feature = "debug-renderer")]
        render(&mut tm, &self.camera, &self.physics.debug_backend.lines);

        if !self.game_finished {
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
                StartPhase::Countdown { go_tick } => {
                    let rem = (go_tick.wrapping_sub(self.sim.tick()) as i32) as f32 * DT;
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
        } else {
            if let Some((order, players)) = &self.results {
                tm.draw_rectangle(0, 0, 320, 180, Color::new(0, 0, 0, 180));

                tm.draw_text("Results", 10, 8, 20, Color::WHITE);

                tm.draw_text("POS", 12, 36, 10, Color::GRAY);
                tm.draw_text("PLAYER", 42, 36, 10, Color::GRAY);
                tm.draw_text("LAPS", 150, 36, 10, Color::GRAY);
                tm.draw_text("TOTAL", 220, 36, 10, Color::GRAY);

                let row_height = 16;
                let max_rows = 7;

                for (i, id) in order.iter().take(max_rows).enumerate() {
                    let Some(result) = players.iter().find(|p| p.id == *id) else {
                        continue;
                    };

                    let y = 50 + i as i32 * row_height;
                    let color = if *id == self.my_id {
                        Color::YELLOW
                    } else {
                        Color::WHITE
                    };

                    let position = format!("{}.", i + 1);
                    tm.draw_text(&position, 12, y, 10, color);

                    let name = format!("Player {}", id);
                    tm.draw_text(&name, 42, y, 10, color);

                    let total = format_time(result.total_time);
                    tm.draw_text(&total, 220, y, 10, color);

                    let laps = result
                        .lap_times
                        .iter()
                        .enumerate()
                        .map(|(lap, time)| format!("{}:{}", lap + 1, format_time(*time)))
                        .collect::<Vec<_>>()
                        .join(" ");

                    tm.draw_text(&laps, 150, y, 8, color);
                }

                tm.draw_text("Race finished", 10, 166, 8, Color::GRAY);
            }
        }

        // let debug = format!(
        //     "rtt {:.0} ms | margin {} (target 2) | clock x{:.3} | lead {} | corr {:.2}px",
        //     self.sim.rtt * 1000.,
        //     self.sim.early,
        //     self.sim.rate(),
        //     self.sim.lead_ticks(),
        //     self.sim.max_err
        // );
        // tm.draw_text(&debug, 10, 40, 10, Color::WHITE);
    }

    fn on_message(
        &mut self,
        _ctx: &mut ctx::Context,
        msg: &wrum_net::proto::ServerMsg,
    ) -> super::Transition {
        match msg {
            ServerMsg::Snapshot { world, early } => self.sim.on_snapshot(world, *early),
            ServerMsg::Relay {
                slot,
                first,
                inputs,
            } => self.sim.on_relay(*slot as usize, *first, inputs),
            ServerMsg::Pong { t0, tick, frac } => self.sim.on_pong(*t0, *tick, *frac),
            ServerMsg::PlayerJoined { slot, .. } => self.sim.on_player_changed(*slot as usize),
            ServerMsg::PlayerLeft { slot } => {
                self.sim.on_player_changed(*slot as usize);
                self.cars.remove(slot);
            }
            ServerMsg::Countdown { go_tick } => {
                self.phase = StartPhase::Countdown { go_tick: *go_tick };
                self.sim.go_tick = Some(*go_tick);
            }
            ServerMsg::RaceReset => {
                self.reset_race();
            }
            ServerMsg::Race {
                slot,
                lap,
                next: _,
                finished,
                lap_time: _,
            } => {
                if *slot == self.my_id {
                    self.lap = *lap;
                    if *finished {
                        self.finished = true;
                        if self.following == self.my_id {
                            if let Some(next) = self.next_unfinished() {
                                self.following = next;
                            }
                        }
                    }
                } else if *finished {
                    self.remote_finished.insert(*slot);
                    if self.following == *slot {
                        if let Some(next) = self.next_unfinished() {
                            self.following = next;
                        }
                    }
                }
            }
            ServerMsg::Positions { order } => {
                self.order = order.clone();
            }
            ServerMsg::LobbyState { host, ready } => {
                self.host = *host;
                self.ready = ready.iter().copied().collect();
            }
            ServerMsg::Results { order, players } => {
                self.sim.go_tick = None;
                if let Some(following) = self.cars.get(&self.following) {
                    self.last_camera_pos = (following.p, following.a);
                }
                self.results = Some((order.clone(), players.clone()));
                self.game_finished = true;
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
        steer: rl.is_key_down(KeyboardKey::KEY_D) as i8 - rl.is_key_down(KeyboardKey::KEY_A) as i8,
        throttle: rl.is_key_down(KeyboardKey::KEY_W) as i8
            - rl.is_key_down(KeyboardKey::KEY_S) as i8,
        drift: rl.is_key_down(KeyboardKey::KEY_SPACE),
    }
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

fn format_time(seconds: f32) -> String {
    let centiseconds = (seconds.max(0.0) * 100.0).round() as u32;
    let minutes = centiseconds / 6000;
    let secs = (centiseconds / 100) % 60;
    let cs = centiseconds % 100;

    format!("{minutes}:{secs:02}.{cs:02}")
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
