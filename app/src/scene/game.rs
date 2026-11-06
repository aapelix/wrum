use std::collections::{HashMap, HashSet};

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
    replay::{Replay, ReplayPlayback},
    sim::{DT, Input, MAX_CARS, Status, V2, grid},
};
use wrum_proto::{ClientMsg, MapKind, PlayerResult, ServerMsg};

use crate::{
    assets::Assets,
    camera::Camera,
    car::Car,
    ctx,
    fx::Fx,
    map::Map,
    scene::{Scene, Transition, basic::DisconnectedScene, menu::MenuScene},
    sim::NetSim,
    ui::{Button, input::mouse_canvas},
};

const MENU: (f32, f32, f32) = (244., 14., 62.);
const NEW_GAME: (f32, f32, f32) = (182., 14., 62.);

#[derive(Clone, Copy)]
enum StartPhase {
    Waiting,
    Countdown { go_tick: u32 },
    Racing,
}

#[derive(Clone, Copy)]
pub struct Pose {
    pub pos: V2,
    pub rot: f32,
    pub z: f32,
    pub status: Status,
    pub invuln: u8,
    pub drift_dir: i8,
    pub tier: u8,
    pub charge: u16,
    pub boost: u8,
    pub boost_lvl: u8,
    pub heat: u8,
    pub over: u8,
    pub perfect: bool,
    pub draft: bool,
    pub scrape: bool,
}
struct Ghost {
    playback: ReplayPlayback,
    slot: u8,
    car: Car,

    previous: Pose,
    current: Pose,

    acc: f32,
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
    last_lap: f32,

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

    last_tier: u8,
    last_status: Status,
    last_perfect: bool,
    last_charge: u16,
    perfect_at: f32,

    names: HashMap<PlayerId, String>,

    button: Button,

    requested_new: bool,

    ghost: Option<Ghost>,
    level_id: String,

    map_kind: MapKind,

    #[cfg(feature = "debug-renderer")]
    show_debug: bool,
}

impl GameScene {
    pub fn new(
        ctx: &mut ctx::Context,
        my_id: PlayerId,
        players: &Vec<(PlayerId, String, [char; 2])>,
        code: &str,
        private: bool,
        map_path: &str,
        map_kind: MapKind,
    ) -> Self {
        let json_path = format!("{map_path}.json");
        let png_path = format!("{map_path}.png");

        let level = Level::load(&json_path).unwrap();
        let map = Map::load(ctx, &png_path, &json_path, 1);

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
            last_tier: 0,
            last_status: Status::Normal,
            last_perfect: false,
            last_charge: 0,
            perfect_at: -10.0,
            names: players
                .iter()
                .map(|(id, name, _)| (*id, name.clone()))
                .collect(),
            button: Button::new(ctx),
            last_lap: 0.,
            requested_new: false,
            ghost: None,
            level_id: map_path.into(),
            map_kind,
            #[cfg(feature = "debug-renderer")]
            show_debug: false,
        };
        s
    }
    pub fn replay(&mut self, ctx: &mut ctx::Context, slot: u8, data: &[u8]) {
        let replay = match Replay::decode(data) {
            Ok(replay) if replay.level == self.level_id => replay,
            Ok(_) => return,
            Err(e) => {
                eprintln!("replay decode failed: {e:?}");
                return;
            }
        };

        let mut playback = match ReplayPlayback::new(replay) {
            Ok(playback) => playback,
            Err(e) => {
                eprintln!("invalid replay: {e:?}");
                return;
            }
        };

        let slot_index = slot as usize;
        if slot_index >= MAX_CARS {
            return;
        }

        let Some(mut current) = replay_pose(&playback, slot_index) else {
            return;
        };

        let mut previous = current;

        let elapsed = if matches!(self.phase, StartPhase::Racing) {
            (self.clock - self.go_at).max(0.0)
        } else {
            0.0
        };

        let target_tick = (elapsed / DT).floor() as u32;

        while playback.tick() < target_tick && !playback.finished() {
            if !playback.step(&self.sim.map) {
                break;
            }

            previous = current;

            if let Some(pose) = replay_pose(&playback, slot_index) {
                current = pose;
            }
        }

        let mut car = Car::new(
            ctx,
            Vector2 {
                x: current.pos.x,
                y: current.pos.y,
            },
            current.rot.to_degrees(),
            0.0,
        );

        apply_pose(&mut car, self.clock, &current);

        let acc = if playback.finished() {
            0.0
        } else {
            (elapsed - playback.tick() as f32 * DT).clamp(0.0, DT)
        };

        self.ghost = Some(Ghost {
            playback,
            slot,
            car,
            previous,
            current,
            acc,
        });
    }

    fn ensure_car(&mut self, ctx: &mut ctx::Context, id: PlayerId, pos: V2, rot_rad: f32) {
        self.cars.entry(id).or_insert_with(|| {
            Car::new(
                ctx,
                Vector2 { x: pos.x, y: pos.y },
                rot_rad.to_degrees(),
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
        self.sim.update(dt, input, &mut out);
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
                    draft: c.drafting(),
                    scrape: c.wall_hit,
                };
                if let Some(car) = self.cars.get_mut(&id) {
                    apply_pose(car, self.clock, &pose);
                }
            }
        }

        if matches!(self.phase, StartPhase::Racing) {
            let map = &self.sim.map;

            if let Some(ghost) = &mut self.ghost {
                ghost.acc += dt.min(0.25);

                while ghost.acc >= DT && !ghost.playback.finished() {
                    ghost.acc -= DT;

                    if !ghost.playback.step(map) {
                        ghost.acc = 0.0;
                        break;
                    }

                    ghost.previous = ghost.current;

                    if let Some(pose) = replay_pose(&ghost.playback, ghost.slot as usize) {
                        ghost.current = pose;
                    }
                }

                let alpha = if ghost.playback.finished() {
                    1.0
                } else {
                    (ghost.acc / DT).clamp(0.0, 1.0)
                };

                let pose = interpolate_pose(ghost.previous, ghost.current, alpha);

                apply_pose(&mut ghost.car, self.clock, &pose);
            }
        }

        let mut fx_cars = self.cars.clone();

        if let Some(ghost) = &self.ghost {
            fx_cars.insert(PlayerId::MAX, ghost.car.clone());
        }

        let map = &self.map;
        self.fx
            .update(dt, &fx_cars, &self.level, &|p| map.is_water(p));

        if !self.game_finished {
            if let Some(c) = self.cars.get(&self.following) {
                self.camera.set_boost(c.boost);
                if c.tier > self.last_tier {
                    self.camera.add_shake(0.25);
                }
                if c.drift_dir != 0 && c.charge + 30 < self.last_charge {
                    self.camera.add_shake(0.35);
                }
                if c.perfect && !self.last_perfect {
                    self.camera.add_shake(0.6);
                    self.perfect_at = self.clock;
                }
                self.last_perfect = c.perfect;
                self.last_charge = c.charge;
                if c.status == Status::Spun && self.last_status != Status::Spun {
                    self.camera.add_shake(0.8);
                }
                self.last_tier = c.tier;
                self.last_status = c.status;
            }

            if matches!(self.phase, StartPhase::Racing) {
                if !self.requested_new && ctx.rl.is_key_pressed(KeyboardKey::KEY_O) {
                    self.requested_new = true;
                    if self.private {
                        ctx.net
                            .send(&ClientMsg::CreateLobby(self.map_kind))
                            .unwrap();
                    } else {
                        ctx.net.send(&ClientMsg::JoinPublic(self.map_kind)).unwrap();
                    }
                }
            }
        } else {
            let mp = mouse_canvas(ctx.rl).into();
            let pressed = ctx
                .rl
                .is_mouse_button_pressed(raylib::ffi::MouseButton::MOUSE_BUTTON_LEFT);

            if pressed {
                if self.button.inside(MENU.0, MENU.1, MENU.2, mp) {
                    ctx.net.send(&ClientMsg::LeaveLobby).unwrap();
                    return Transition::Switch(Box::new(MenuScene::new("", ctx)));
                } else if !self.private
                    && !self.requested_new
                    && self.button.inside(NEW_GAME.0, NEW_GAME.1, NEW_GAME.2, mp)
                {
                    self.requested_new = true;
                    ctx.net
                        .send(&ClientMsg::JoinPublic(MapKind::Untitled))
                        .unwrap();
                }
            }
        }

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
        let mp = mouse_canvas(ctx.rl);
        let pressed = ctx
            .rl
            .is_mouse_button_down(raylib::ffi::MouseButton::MOUSE_BUTTON_LEFT);

        self.fx.flush(ctx);
        {
            let mut tm = ctx
                .rl
                .begin_texture_mode(&ctx.thread, &mut self.shadow_canvas);
            tm.clear_background(Color::BLANK);
            self.map.draw_shadows(ctx.assets, &mut tm, &self.camera);

            for (_, car) in &self.cars {
                car.draw_shadow(ctx.assets, &mut tm, &self.camera);
            }

            if let Some(ghost) = &self.ghost {
                ghost.car.draw_shadow(ctx.assets, &mut tm, &self.camera);
            }
        }

        let mut tm = ctx.rl.begin_texture_mode(ctx.thread, ctx.canvas);
        tm.clear_background(Color::new(20, 160, 46, 255));

        self.map.draw_layers(ctx.assets, &mut tm, &self.camera);
        self.fx.draw_marks(&mut tm, &self.camera);
        tm.draw_texture_rec(
            self.shadow_canvas.texture(),
            Rectangle::new(0., 0., 320., -180.),
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
            if !Map::visible(prop, bounds) {
                continue;
            }

            let (pos, _) = self.camera.apply(prop.pos, prop.a);
            drawables.push((pos.y, i, Drawable::Prop));
        }
        if let Some(ghost) = &self.ghost {
            let (pos, _) = self.camera.apply(ghost.car.p, ghost.car.a);
            drawables.push((pos.y, usize::MAX, Drawable::Ghost));
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
                            map.draw_water_overlay(ctx.assets, tm, cam, pos, depth);
                        },
                        false,
                    );
                }
                Drawable::Prop => {
                    self.map.draw_prop(ctx.assets, &mut tm, i, &self.camera);
                }
                Drawable::Ghost => {
                    let ghost = self.ghost.as_ref().unwrap();
                    let car = &ghost.car;
                    let (map, cam) = (&mut self.map, &self.camera);
                    car.draw(
                        ctx.assets,
                        &mut tm,
                        &self.camera,
                        self.clock,
                        |tm, pos, depth| {
                            map.draw_water_overlay(ctx.assets, tm, cam, pos, depth);
                        },
                        true,
                    );
                }
            }
        }

        for (&id, car) in &self.cars {
            if id == self.my_id {
                continue;
            }
            let Some(name) = self.names.get(&id) else {
                continue;
            };

            let (pos, _) = self.camera.apply(car.p, car.a);

            let text_size = 8;
            let width = tm.measure_text(name, text_size);
            let x = pos.x as i32 - width / 2;
            let y = pos.y as i32 - 20 - car.z as i32;

            tm.draw_text(name, x, y, text_size, Color::WHITE);
        }

        self.fx.draw_flames(&mut tm, &self.camera);
        self.fx.draw_bits(&mut tm, &self.camera);
        self.fx.draw_puffs(&mut tm, &self.camera);
        let map = &self.map;
        self.fx
            .draw_droplets(&mut tm, &self.camera, &|p| map.is_water(p));

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

                    let text = format_time(0.);
                    tm.draw_text(&text, 14, 14, 20, Color::BLACK);
                    tm.draw_text(&text, 12, 12, 20, Color::WHITE);
                }
                StartPhase::Racing => {
                    if self.clock - self.go_at < 1.0 {
                        big_text(&mut tm, "GO", 48);
                    }

                    let text = format_time(self.clock - self.go_at);
                    tm.draw_text(&text, 14, 14, 20, Color::BLACK);
                    tm.draw_text(&text, 12, 12, 20, Color::WHITE);

                    if self.last_lap != 0. {
                        let text = format_time(self.last_lap);
                        tm.draw_text(&text, 13, 33, 10, Color::BLACK);
                        tm.draw_text(&text, 12, 32, 10, Color::WHITE);
                    }
                }
            }
            if matches!(self.phase, StartPhase::Racing) {
                if let Some(c) = self.cars.get(&self.my_id) {
                    draw_drift_hud(&mut tm, c, self.clock, self.perfect_at);
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
                &self.names,
                &self.button,
                self.private,
                ctx.assets,
                mp.into(),
                pressed,
            );
        }

        #[cfg(feature = "debug-renderer")]
        if self.show_debug {
            debug_draw(&mut tm, &self.camera, &self.sim);
            let labels = ["RTT", "Marg", "Clock", "Lead", "Corr"];
            let values = [
                format!("{:>4.0} ms", self.sim.rtt * 1000.),
                format!("{:>3} (target {:.1})", self.sim.early, self.sim.margin()),
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
        msg: &wrum_proto::ServerMsg,
    ) -> super::Transition {
        match msg {
            ServerMsg::Reject(reason) => {
                println!("reject: {reason:#?}")
            }
            ServerMsg::Snapshot { world, early } => self.sim.on_snapshot(world, *early),
            ServerMsg::Relay {
                slot,
                first,
                inputs,
            } => self.sim.on_relay(*slot as usize, *first, inputs),
            ServerMsg::Pong { t0, tick, frac } => self.sim.on_pong(*t0, *tick, *frac),
            ServerMsg::PlayerJoined { slot, name, .. } => {
                self.names.insert(*slot, name.clone());
                self.sim.on_player_changed(*slot as usize)
            }
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
                lap_time,
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
                    if let Some(lap_time) = lap_time {
                        self.last_lap = *lap_time;
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
            ServerMsg::Welcome {
                slot,
                players,
                code,
                private,
                map,
                map_kind,
                ..
            } => {
                return super::Transition::Switch(Box::new(GameScene::new(
                    ctx, *slot, players, code, *private, map, *map_kind,
                )));
            }
            _ => {}
        }
        super::Transition::None
    }
    fn on_disconnect(&mut self, _ctx: &mut ctx::Context) -> Transition {
        Transition::Switch(Box::new(DisconnectedScene))
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
    Ghost,
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
    names: &HashMap<PlayerId, String>,
    button: &Button,
    private: bool,
    assets: &Assets,
    mp: Vector2,
    pressed: bool,
) {
    const GOLD: Color = Color::new(255, 215, 64, 255);
    const SILVER: Color = Color::new(200, 205, 215, 255);
    const BRONZE: Color = Color::new(205, 127, 80, 255);
    const PURPLE: Color = Color::new(190, 120, 255, 255);

    let bg = (t / 0.25).clamp(0.0, 1.0);
    tm.draw_rectangle(0, 0, 320, 180, fade(Color::new(0, 0, 0, 200), bg));

    tm.draw_text("RESULTS", 14, 10, 20, fade(Color::WHITE, bg));
    tm.draw_rectangle(14, 32, 292, 1, fade(Color::GRAY, bg));

    button.draw(
        assets, tm, MENU.0, MENU.1, MENU.2, mp, pressed, "Menu", true, false,
    );
    if !private {
        button.draw(
            assets, tm, NEW_GAME.0, NEW_GAME.1, NEW_GAME.2, mp, pressed, "New game", true, false,
        );
    }

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
        let Some(name) = names.get(id) else {
            return;
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

pub fn draw_drift_hud(
    tm: &mut RaylibTextureMode<'_, '_, RaylibHandle>,
    c: &Car,
    now: f32,
    perfect_at: f32,
) {
    use wrum_core::sim::{CHARGE_TIERS, HEAT_MAX, OVER_TICKS};

    const TIER_COL: [Color; 4] = [
        Color::new(230, 230, 235, 255),
        Color::new(60, 160, 255, 255),
        Color::new(255, 150, 40, 255),
        Color::new(190, 90, 255, 255),
    ];
    let (x, y, w, h) = (12, 160, 70, 5);
    let max = CHARGE_TIERS[2] as f32;

    if c.drift_dir != 0 {
        tm.draw_rectangle(x - 1, y - 1, w + 2, h + 2, Color::new(0, 0, 0, 170));
        let frac = (c.charge as f32 / max).clamp(0.0, 1.0);
        let mut col = TIER_COL[c.tier as usize];
        if c.over > 20 && ((now * 14.0) as i32) % 2 == 0 {
            col = Color::WHITE;
        }
        tm.draw_rectangle(x, y, (w as f32 * frac) as i32, h, col);
        for t in CHARGE_TIERS {
            let tx = x + (w as f32 * t as f32 / max) as i32;
            tm.draw_rectangle(tx, y, 1, h, Color::new(0, 0, 0, 200));
        }
        if c.over > 0 {
            let f = 1.0 - c.over as f32 / OVER_TICKS as f32;
            tm.draw_rectangle(
                x,
                y + h + 1,
                (w as f32 * f) as i32,
                1,
                Color::new(255, 80, 60, 255),
            );
        }
    }

    if c.heat > 8 {
        let hy = y + h + 3;
        let f = c.heat as f32 / HEAT_MAX as f32;
        tm.draw_rectangle(x - 1, hy - 1, w + 2, 4, Color::new(0, 0, 0, 140));
        let hot = Color::new(255, (200.0 - 150.0 * f) as u8, 40, 255);
        tm.draw_rectangle(x, hy, (w as f32 * f) as i32, 2, hot);
    }

    let since = now - perfect_at;
    if since < 0.8 {
        let a = (1.0 - since / 0.8).clamp(0.0, 1.0);
        let lift = (since * 14.0) as i32;
        let s = "PERFECT!";
        let sw = tm.measure_text(s, 10);
        let gold = fade(Color::new(255, 215, 64, 255), a);
        tm.draw_text(
            s,
            160 - sw / 2 + 1,
            120 - lift + 1,
            10,
            fade(Color::BLACK, a),
        );
        tm.draw_text(s, 160 - sw / 2, 120 - lift, 10, gold);
    }
}

fn sample_input(rl: &RaylibHandle) -> Input {
    Input {
        steer: rl.is_key_down(KeyboardKey::KEY_D) as i8 - rl.is_key_down(KeyboardKey::KEY_A) as i8,
        throttle: rl.is_key_down(KeyboardKey::KEY_W) as i8
            - rl.is_key_down(KeyboardKey::KEY_S) as i8,
        drift: rl.is_key_down(KeyboardKey::KEY_SPACE),
    }
}
pub fn apply_pose(car: &mut Car, now: f32, p: &Pose) {
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

    car.drift_dir = p.drift_dir;
    car.tier = p.tier;
    car.charge = p.charge;
    car.boost = p.boost;
    car.boost_lvl = p.boost_lvl;
    car.heat = p.heat;
    car.over = p.over;
    car.perfect = p.perfect;
    car.draft = p.draft;
    car.scrape = p.scrape;
}

pub fn format_time(seconds: f32) -> String {
    let centiseconds = (seconds.max(0.0) * 100.0).round() as u32;
    let minutes = centiseconds / 6000;
    let secs = (centiseconds / 100) % 60;
    let cs = centiseconds % 100;

    format!("{minutes}:{secs:02}.{cs:02}")
}

fn replay_pose(playback: &ReplayPlayback, slot: usize) -> Option<Pose> {
    let c = playback.world().cars[slot];

    c.active.then_some(Pose {
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
        draft: c.drafting(),
        scrape: c.wall_hit,
    })
}
fn interpolate_pose(a: Pose, b: Pose, t: f32) -> Pose {
    Pose {
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
    }
}
fn lerp_angle(a: f32, b: f32, t: f32) -> f32 {
    use std::f32::consts::{PI, TAU};

    let delta = (b - a + PI).rem_euclid(TAU) - PI;
    a + delta * t
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
    for p in &map.speed_pads {
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
