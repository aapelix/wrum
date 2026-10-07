use std::collections::VecDeque;

use raylib::{
    RaylibHandle,
    ffi::KeyboardKey,
    math::{Vector2, lerp},
    texture::RenderTexture2D,
};
use shared::{
    coll::{Obb, Vec2},
    interp::{INTERP_DELAY_TICKS, INTERP_SMOOTH, INTERP_SNAP_TICKS, SnapshotBuffer, lerp_angle},
    net::{Channel, ClientMessage, proto::Snapshot},
    sim::{CAR_HALFS, CAR_RESTITUTION, CarState, Input, TICK_DT},
};

use crate::{camera::Camera, entity::car::Car, scene::Scene};

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
}

pub struct GameScene {
    shadow_c: RenderTexture2D,
    camera: Camera,
    // map: Map,
    // level: Level
    //
    cars: Vec<Car>,

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
    fn ensure_car(&mut self, ctx: &mut crate::ctx::Context, id: u8, spawn: &CarState) {
        if self.cars.get(id as usize).is_some() {
            return;
        }
        let mut car = Car::new(
            ctx,
            Vector2 {
                x: spawn.pos.x,
                y: spawn.pos.y,
            },
            spawn.a,
            spawn.t,
        );
        car.sim = *spawn;
        self.cars.push(car);
    }

    fn sim_tick(&mut self, ctx: &mut crate::ctx::Context) {
        let mut input = sample_input(&ctx.rl);
        input.seq = ctx.net.next_seq;
        ctx.net.next_seq += 1;

        // sim

        let my_id = self.my_id;
        let ghosts: Vec<Obb> = self
            .cars
            .iter()
            .enumerate()
            .filter(|(i, _)| *i != my_id as usize)
            .map(|(_, car)| Obb {
                pos: Vec2 {
                    x: car.p.x,
                    y: car.p.y,
                },
                halfs: CAR_HALFS,
                a: car.a.to_radians(),
            })
            .collect();
        let me = &mut self.cars[my_id as usize];

        self.prev_sim = me.sim;
        me.sim.step(&input);
        // level_resolve_car(g->level, &me->sim);

        for ghost in ghosts {
            me.sim.resolve_vs_obstacle(ghost, CAR_RESTITUTION);
        }

        // input

        self.pending.push(input);
        ctx.net
            .send(
                Channel::Unreliable,
                &ClientMessage::Input {
                    inputs: self.pending.items.clone().into(),
                },
            )
            .unwrap();
    }
    fn apply_snapshot(&mut self, ctx: &mut crate::ctx::Context, snap: &mut Snapshot) {
        self.last_server_tick = snap.server_tick;

        for (id, state) in &mut snap.ents {
            self.ensure_car(ctx, *id, state);

            if *id != self.my_id {
                continue;
            }

            self.pending.discard_upto(snap.ack_seq);

            let my_id = self.my_id as usize;
            let before = self.cars[my_id].sim;
            let mut sim = *state;

            let ghosts: Vec<Obb> = self
                .cars
                .iter()
                .enumerate()
                .filter(|(i, _)| *i != my_id)
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

            self.cars[my_id].sim = sim;

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

        for (id, car) in self.cars.iter_mut().enumerate() {
            let id = id as u8;
            if id == self.my_id {
                return;
            }

            let sa = a.find_entity(id);
            let sc = c.find_entity(id);

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
            car.a = r.a.to_radians();
            car.t = r.t.to_radians();
        }
    }
    fn update_own_reder(&mut self, dt: f32) {
        let me = &mut self.cars[self.my_id as usize];

        let alpha = (self.acc / TICK_DT).clamp(0., 1.);
        let ix = lerp(self.prev_sim.pos.x, me.sim.pos.x, alpha);
        let iy = lerp(self.prev_sim.pos.y, me.sim.pos.y, alpha);
        let ia = lerp_angle(self.prev_sim.a, me.sim.a, alpha);
        let it = lerp(self.prev_sim.t, me.sim.t, alpha);

        let decay = (-RECON_DECAY * dt).exp();
        self.err_p.x *= decay;
        self.err_p.x *= decay;
        self.err_a *= decay;

        me.p.x = ix + self.err_p.x;
        me.p.y = iy + self.err_p.y;
        me.a = (ia + self.err_a).to_radians();
        me.t = it.to_radians();
    }
}

impl Scene for GameScene {
    fn update(&mut self, ctx: &mut crate::ctx::Context, dt: f32) -> super::Transition {
        self.acc += dt;

        super::Transition::None
    }
    fn draw(&mut self, ctx: &mut crate::ctx::Context) {}

    fn blocks_update(&self) -> bool {
        true
    }
    fn blocks_draw(&self) -> bool {
        true
    }
}

fn sample_input(rl: &RaylibHandle) -> Input {
    Input {
        seq: 0,
        steer: rl.is_key_down(KeyboardKey::KEY_D) as i8 - rl.is_key_down(KeyboardKey::KEY_A) as i8,
        thr: rl.is_key_down(KeyboardKey::KEY_W) as i8 - rl.is_key_down(KeyboardKey::KEY_S) as i8,
    }
}
