use std::collections::{HashMap, HashSet};

#[cfg(feature = "debug-renderer")]
use crate::sim::MARGIN;
use raylib::{
    RaylibHandle,
    drawing::{RaylibDraw, RaylibTextureMode, RaylibTextureModeExt},
    ffi::{Color, KeyboardKey, Vector2},
    math::Rectangle,
    texture::{RaylibRenderTexture2D, RenderTexture2D},
};
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
    loading_fade: f32,

    results: Option<(Vec<PlayerId>, Vec<PlayerResult>)>,
    results_at: f32,
    reset_tick: u32,
    results_wait: f32,

    level: Level,
    fx: Fx,

    #[cfg(feature = "debug-renderer")]
    show_debug: bool,
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
        let map = Map::load(
            ctx,
            "assets/maps/untitled.png",
            "assets/maps/untitled.json",
            1,
        );

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
            loading_fade: 1.0,
            results: None,
            results_at: 0.,
            reset_tick: 0,
            results_wait: 0.,
            fx,
            level,
            #[cfg(feature = "debug-renderer")]
            show_debug: false,
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

    fn reset_race(&mut self, ctx: &mut ctx::Context) {
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
        self.results_at = 0.;

        self.sim.go_tick = None;
        self.fx.clear(ctx);
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
        } else if let Some(c) = self.cars.get(&self.following) {
            self.camera.update(c.p, c.a, dt);
        }

        if self.sim.have_world() && !self.cars.is_empty() {
            self.loading_fade = (self.loading_fade - dt / 0.5).max(0.0);
        }

        #[cfg(feature = "debug-renderer")]
        if ctx.rl.is_key_pressed(KeyboardKey::KEY_F6) {
            self.show_debug = !self.show_debug;
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

        self.map.draw_layers(ctx.assets, &mut tm, &self.camera);
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
                    car.draw(
                        ctx.assets,
                        &mut tm,
                        &self.camera,
                        self.clock,
                        |tm, pos, depth| {
                            map.draw_water_overlay(tm, cam, pos, depth);
                        },
                    );
                }
                Drawable::Prop => {
                    self.map.draw_prop(ctx.assets, &mut tm, i, &self.camera);
                }
            }
        }
        self.fx.draw_puffs(&mut tm, &self.camera);

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
        } else if let Some((order, players)) = &self.results {
            let rem = (self.reset_tick.wrapping_sub(self.sim.tick()) as i32) as f32 * DT;
            draw_results(
                &mut tm,
                order,
                players,
                self.my_id,
                self.clock - self.results_at,
                rem,
                self.results_wait,
            );
        }

        #[cfg(feature = "debug-renderer")]
        if self.show_debug {
            debug_draw(&mut tm, &self.camera, &self.sim);
            let labels = ["RTT", "Marg", "Clock", "Lead", "Corr"];
            let values = [
                format!("{:>4.0} ms", self.sim.rtt * 1000.),
                format!("{:>3} (target {MARGIN})", self.sim.early),
                format!("x{:.3}", self.sim.rate()),
                format!("{:>3} ticks", self.sim.lead_ticks()),
                format!("{:.2} px", self.sim.max_err),
            ];

            tm.draw_rectangle(5, 5, 105, 57, Color::new(0, 0, 0, 180));
            for (i, (label, value)) in labels.iter().zip(&values).enumerate() {
                let y = 9 + i as i32 * 10;
                tm.draw_text(label, 9, y, 8, Color::WHITE);
                text_right(&mut tm, value, 105, y, 8, Color::WHITE);
            }
        };

        if self.loading_fade > 0.0 {
            tm.draw_rectangle(0, 0, 320, 180, fade(Color::BLACK, self.loading_fade));
        }
    }

    fn on_message(
        &mut self,
        ctx: &mut ctx::Context,
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
                self.reset_race(ctx);
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
            ServerMsg::Results {
                order,
                players,
                reset_tick,
            } => {
                if let Some(following) = self.cars.get(&self.following) {
                    self.last_camera_pos = (following.p, following.a);
                }
                self.reset_tick = *reset_tick;
                self.results_wait =
                    (reset_tick.wrapping_sub(self.sim.tick()) as i32).max(1) as f32 * DT;
                self.results_at = self.clock;
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
fn fade(c: Color, a: f32) -> Color {
    Color::new(c.r, c.g, c.b, (c.a as f32 * a.clamp(0.0, 1.0)) as u8)
}
fn text_right(
    tm: &mut RaylibTextureMode<'_, '_, RaylibHandle>,
    s: &str,
    right: i32,
    y: i32,
    size: i32,
    c: Color,
) {
    let w = tm.measure_text(s, size);
    tm.draw_text(s, right - w, y, size, c);
}

fn draw_results(
    tm: &mut RaylibTextureMode<'_, '_, RaylibHandle>,
    order: &[PlayerId],
    players: &[PlayerResult],
    my_id: PlayerId,
    t: f32,
    rem: f32,
    wait: f32,
) {
    const GOLD: Color = Color::new(255, 215, 64, 255);
    const SILVER: Color = Color::new(200, 205, 215, 255);
    const BRONZE: Color = Color::new(205, 127, 80, 255);
    const PURPLE: Color = Color::new(190, 120, 255, 255);

    let bg = (t / 0.25).clamp(0.0, 1.0);
    tm.draw_rectangle(0, 0, 320, 180, fade(Color::new(0, 0, 0, 200), bg));

    tm.draw_text("RESULTS", 14, 10, 20, fade(Color::WHITE, bg));
    tm.draw_rectangle(14, 32, 292, 1, fade(Color::GRAY, bg));

    let hc = fade(Color::GRAY, bg);
    tm.draw_text("POS", 14, 37, 8, hc);
    tm.draw_text("PLAYER", 40, 37, 8, hc);
    text_right(tm, "BEST LAP", 215, 37, 8, hc);
    text_right(tm, "TOTAL", 306, 37, 8, hc);

    let fastest = players
        .iter()
        .flat_map(|p| p.lap_times.iter().copied())
        .fold(f32::INFINITY, f32::min);

    let winner_total = order
        .first()
        .and_then(|id| players.iter().find(|p| p.id == *id))
        .map(|p| p.total_time)
        .unwrap_or(0.0);

    let (row_h, max_rows, top) = (14, 7, 48);

    for (i, id) in order.iter().take(max_rows).enumerate() {
        let Some(r) = players.iter().find(|p| p.id == *id) else {
            continue;
        };

        let p = ((t - 0.2 - i as f32 * 0.1) / 0.25).clamp(0.0, 1.0);
        if p <= 0.0 {
            continue;
        }
        let ease = 1.0 - (1.0 - p) * (1.0 - p);
        let dx = ((1.0 - ease) * 40.0) as i32;
        let y = top + i as i32 * row_h;
        let me = *id == my_id;

        if me {
            tm.draw_rectangle(
                10 + dx,
                y - 2,
                300,
                row_h - 1,
                fade(Color::new(255, 255, 255, 40), p),
            );
        }

        let pos_col = match i {
            0 => GOLD,
            1 => SILVER,
            2 => BRONZE,
            _ => Color::WHITE,
        };
        let txt = if me { Color::YELLOW } else { Color::WHITE };

        tm.draw_text(&format!("{}", i + 1), 16 + dx, y, 10, fade(pos_col, p));
        let name = if me {
            format!("Player {} (you)", id)
        } else {
            format!("Player {}", id)
        };
        tm.draw_text(&name, 40 + dx, y, 10, fade(txt, p));

        if let Some(best) = r.lap_times.iter().copied().reduce(f32::min) {
            let c = if (best - fastest).abs() < 1e-4 {
                PURPLE
            } else {
                txt
            };
            text_right(tm, &format_time(best), 215 + dx, y, 10, fade(c, p));
        }

        if i == 0 {
            text_right(
                tm,
                &format_time(r.total_time),
                306 + dx,
                y,
                10,
                fade(txt, p),
            );
        } else {
            let gap = format!("+{:.2}", (r.total_time - winner_total).max(0.0));
            text_right(tm, &gap, 306 + dx, y, 10, fade(txt, p));
        }
    }

    if t > 0.2 + max_rows as f32 * 0.1 {
        if let Some(me) = players.iter().find(|p| p.id == my_id) {
            let a = ((t - 1.0) / 0.3).clamp(0.0, 1.0);
            tm.draw_rectangle(14, 150, 292, 1, fade(Color::GRAY, a));
            let laps = me
                .lap_times
                .iter()
                .enumerate()
                .map(|(i, l)| format!("L{} {}", i + 1, format_time(*l)))
                .collect::<Vec<_>>()
                .join("   ");
            tm.draw_text(&laps, 14, 155, 8, fade(Color::WHITE, a));
        }
    }

    let s = if rem > 0.0 {
        format!("Next race in {}", rem.ceil() as i32)
    } else {
        "Starting...".to_string()
    };
    let w = tm.measure_text(&s, 10);
    tm.draw_text(&s, 160 - w / 2, 164, 10, fade(Color::WHITE, bg));

    let frac = (1.0 - rem / wait).clamp(0.0, 1.0);
    tm.draw_rectangle(14, 176, 292, 2, fade(Color::new(255, 255, 255, 50), bg));
    tm.draw_rectangle(14, 176, (292.0 * frac) as i32, 2, fade(Color::WHITE, bg));
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
fn debug_draw(tm: &mut RaylibTextureMode<'_, '_, RaylibHandle>, cam: &Camera, sim: &NetSim) {
    use wrum_core::sim::{RADIUS, Status};

    let line = |tm: &mut RaylibTextureMode<'_, '_, RaylibHandle>, a: V2, b: V2, c: Color| {
        let (a, _) = cam.apply(Vector2 { x: a.x, y: a.y }, 0.0);
        let (b, _) = cam.apply(Vector2 { x: b.x, y: b.y }, 0.0);
        tm.draw_line_v(a, b, c);
    };

    let map = sim.map();

    // obstacles
    for s in &map.segs {
        line(tm, s.a, s.b, Color::RED);
    }
    // checkpoints
    for g in &map.gates {
        line(tm, g.a, g.b, Color::YELLOW);
    }
    // pads
    for p in &map.pads {
        let c = p.corners();
        for i in 0..4 {
            line(tm, c[i], c[(i + 1) % 4], Color::ORANGE);
        }
    }

    // cars
    for car in sim.w.cars.iter().filter(|c| c.active) {
        let col = match car.status {
            Status::Normal if car.invuln > 0 => Color::SKYBLUE,
            Status::Normal => Color::LIME,
            Status::Spun => Color::MAGENTA,
            Status::Drowning => Color::BLUE,
        };
        for p in car.circles() {
            let (p, _) = cam.apply(Vector2 { x: p.x, y: p.y }, 0.0);
            tm.draw_circle_lines(p.x as i32, p.y as i32, RADIUS, col);
        }
        line(tm, car.pos, car.pos + car.vel * 0.2, Color::WHITE);
        line(tm, car.safe, car.safe + V2::new(0.0, -4.0), Color::GREEN);
    }
}
