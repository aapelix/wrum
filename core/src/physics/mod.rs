use std::collections::HashMap;

pub use glam::Vec2;
#[cfg(feature = "debug-renderer")]
pub use rapier2d::pipeline::DebugColor;
#[cfg(feature = "debug-renderer")]
use rapier2d::pipeline::{
    DebugRenderBackend, DebugRenderMode, DebugRenderPipeline, DebugRenderStyle,
};
pub use rapier2d::{
    dynamics::{
        CCDSolver, ImpulseJointSet, IntegrationParameters, IslandManager, MultibodyJointSet,
        RigidBodyBuilder, RigidBodyHandle, RigidBodySet, SoftBodySet,
    },
    geometry::{BroadPhaseBvh, ColliderBuilder, ColliderSet, NarrowPhase},
    math::Vector,
    pipeline::PhysicsPipeline,
};
use rapier2d::{
    geometry::{ColliderHandle, Group, InteractionGroups},
    math::Rotation,
};
use rkyv::{Archive, Deserialize, Serialize};

use crate::{PlayerId, TICK_DT, level::Level};

#[derive(Archive, Serialize, Deserialize, Debug, Clone, Copy, Default)]
pub struct Input {
    pub seq: u32,
    pub throttle: i8,
    pub steer: i8,
    pub drift: bool,
}

pub enum Shape {
    Box { hw: f32, hh: f32 },
    Convex { points: Vec<Vec2> },
    Wall { points: Vec<Vec2> },
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum Surface {
    Road = 0,
    Grass = 1,
    Water = 2,
    Pad = 3,
}
struct SurfaceParams {
    max_speed: f32,
    accel: f32,
    grip: f32,
    extra_decel: f32,
}
impl Surface {
    fn params(self) -> SurfaceParams {
        match self {
            Surface::Road => SurfaceParams {
                max_speed: 1.0,
                accel: 1.0,
                grip: 1.0,
                extra_decel: 0.0,
            },
            Surface::Grass => SurfaceParams {
                max_speed: 0.45,
                accel: 0.6,
                grip: 0.7,
                extra_decel: 200.0,
            },
            _ => Self::Road.params(),
        }
    }
}

struct Touch {
    surface: Surface,
    water: bool,
    pad: bool,
}

pub const GRAVITY: f32 = 400.0;

const JUMP_VZ: f32 = 150.0;
const PAD_MIN_SPEED: f32 = 150.0;
const CLEARANCE: f32 = 6.0;
const HIT_MIN_SPEED: f32 = 70.0;
const HIT_HOP: f32 = 60.0;
const STUN_TICKS: u16 = 45;
const INVULN_TICKS: u8 = 60;
const SPIN_RATE: f32 = 18.0;
const WATER_DRAG: f32 = 4.0;
const SINK_ACCEL: f32 = 80.0;
const SINK_DRAG: f32 = 2.5;
const MAX_DEPTH: f32 = 24.0;
const DROWN_TICKS: u16 = 100;

const HALF_WIDTH: f32 = 4.;
const HALF_LENGTH: f32 = 7.5;

const CAR_MASS: f32 = 1.0;
const MAX_SPEED: f32 = 180.0;
const MAX_REVERSE: f32 = 120.0;
const ACCEL: f32 = 220.0;
const BRAKE: f32 = 450.0;
const COAST_DECEL: f32 = 120.0;
const LATERAL_GRIP: f32 = 16.0;
const DRIFT_GRIP: f32 = 5.0;
const TURN_RATE: f32 = 2.8;
const DRIFT_TURN: f32 = TURN_RATE * 1.3;
const TURN_SMOOTH: f32 = 14.0;
const MIN_TURN_SPEED: f32 = 25.0;

const CHECKPOINT_TAG: u128 = 1 << 64;

#[derive(Archive, Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Status {
    #[default]
    Normal,
    Spun,
    Drowning,
}

#[derive(Archive, Serialize, Deserialize, Clone, Copy, Debug, Default)]
pub struct CarState {
    pub pos: Vec2,
    pub rot: f32,
    pub z: f32,
    pub vz: f32,
    pub vel: Vec2,
    pub angular_vel: f32,
    pub ack: u32,
    pub status: Status,
    pub timer: u16,
    pub invuln: u8,
}

#[derive(Default)]
struct CarExtra {
    z: f32,
    vz: f32,
    status: Status,
    timer: u16,
    invuln: u8,
    safe: (Vec2, f32),
    safe_next: (Vec2, f32),
    age: u32,
}

pub struct PhysicsWorld {
    pipeline: PhysicsPipeline,
    integration: IntegrationParameters,
    islands: IslandManager,
    broad_phase: BroadPhaseBvh,
    narrow_phase: NarrowPhase,
    bodies: RigidBodySet,
    colliders: ColliderSet,
    impulse_joints: ImpulseJointSet,
    multibody_joints: MultibodyJointSet,
    soft_bodies: SoftBodySet,
    ccd: CCDSolver,

    cars: HashMap<PlayerId, RigidBodyHandle>,
    car_colliders: HashMap<PlayerId, ColliderHandle>,
    remotes: HashMap<PlayerId, RigidBodyHandle>,
    inputs: HashMap<PlayerId, Input>,
    extras: HashMap<PlayerId, CarExtra>,
    pub authoritative: bool,

    #[cfg(feature = "debug-renderer")]
    debug_render_pipeline: DebugRenderPipeline,
    #[cfg(feature = "debug-renderer")]
    pub debug_backend: LineCollector,
}

impl Default for PhysicsWorld {
    fn default() -> Self {
        let mut integration = IntegrationParameters::default();
        integration.dt = TICK_DT;

        Self {
            pipeline: PhysicsPipeline::new(),
            integration,
            islands: IslandManager::new(),
            broad_phase: BroadPhaseBvh::new(),
            narrow_phase: NarrowPhase::new(),
            bodies: RigidBodySet::new(),
            colliders: ColliderSet::new(),
            impulse_joints: ImpulseJointSet::new(),
            multibody_joints: MultibodyJointSet::new(),
            soft_bodies: SoftBodySet::new(),
            ccd: CCDSolver::new(),
            car_colliders: HashMap::new(),
            cars: HashMap::new(),
            remotes: HashMap::new(),
            inputs: HashMap::new(),
            extras: HashMap::new(),
            authoritative: true,
            #[cfg(feature = "debug-renderer")]
            debug_render_pipeline: DebugRenderPipeline::new(
                DebugRenderStyle::default(),
                DebugRenderMode::COLLIDER_SHAPES | DebugRenderMode::CONTACTS,
            ),
            #[cfg(feature = "debug-renderer")]
            debug_backend: LineCollector { lines: vec![] },
        }
    }
}

impl PhysicsWorld {
    pub fn load_level(&mut self, level: &Level) {
        let mut cut = earcut::Earcut::new();

        for o in &level.obstacles {
            let v = |p: &Vec2| Vector::new(p.x, p.y);
            let builder = match &o.shape {
                Shape::Box { hw, hh } => ColliderBuilder::cuboid(*hw, *hh),
                Shape::Convex { points } => {
                    let pts: Vec<_> = points.iter().map(v).collect();
                    match ColliderBuilder::convex_hull(&pts) {
                        Some(b) => b,
                        None => continue,
                    }
                }
                Shape::Wall { points } => {
                    ColliderBuilder::polyline(points.iter().map(v).collect(), None)
                }
            };
            let collider = builder
                .translation(Vector::new(o.pos.x, o.pos.y))
                .rotation(o.rot)
                .friction(0.0)
                .restitution(0.2)
                .collision_groups(groups(G_SOLID, G_CAR))
                .build();
            self.colliders.insert(collider);
        }
        for z in &level.zones {
            let vertices: Vec<[f64; 2]> =
                z.points.iter().map(|p| [p.x as f64, p.y as f64]).collect();
            let mut indices = Vec::<usize>::new();
            cut.earcut(vertices.iter().copied(), &[] as &[usize], &mut indices);

            for tri in indices.chunks_exact(3) {
                let points = [z.points[tri[0]], z.points[tri[1]], z.points[tri[2]]];

                let Some(builder) =
                    ColliderBuilder::convex_hull(&points.map(|p| Vector::new(p.x, p.y)))
                else {
                    continue;
                };

                self.colliders.insert(
                    builder
                        .translation(Vector::new(z.pos.x, z.pos.y))
                        .rotation(z.rot)
                        .sensor(true)
                        .user_data(z.surface as u128)
                        .collision_groups(groups(G_ZONE, G_CAR))
                        .build(),
                );
            }
        }
        for cp in &level.checkpoints {
            let c = ColliderBuilder::cuboid(cp.hw, cp.hh)
                .translation(Vector::new(cp.pos.x, cp.pos.y))
                .rotation(cp.rot)
                .sensor(true)
                .user_data(CHECKPOINT_TAG | cp.index as u128)
                .collision_groups(groups(G_GATE, G_CAR | G_AIR))
                .build();
            self.colliders.insert(c);
        }
    }

    pub fn spawn_car(&mut self, id: PlayerId, pos: Vec2, a: f32) {
        let body = RigidBodyBuilder::dynamic()
            .translation(Vector::new(pos.x, pos.y))
            .linear_damping(0.)
            .angular_damping(0.)
            .rotation(a)
            .can_sleep(false)
            .ccd_enabled(true)
            .build();
        let handle = self.bodies.insert(body);
        let collider = ColliderBuilder::cuboid(HALF_WIDTH, HALF_LENGTH)
            .mass(CAR_MASS)
            .friction(0.4)
            .restitution(0.1)
            .collision_groups(groups(G_CAR, Group::ALL))
            .build();
        let ch = self
            .colliders
            .insert_with_parent(collider, handle, &mut self.bodies);

        self.car_colliders.insert(id, ch);
        self.cars.insert(id, handle);
        self.inputs.insert(id, Input::default());
        self.extras.insert(
            id,
            CarExtra {
                safe: (pos, a),
                safe_next: (pos, a),
                ..Default::default()
            },
        );
    }
    pub fn spawn_remote(&mut self, id: PlayerId, s: &CarState) {
        let body = RigidBodyBuilder::kinematic_position_based()
            .translation(Vector::new(s.pos.x, s.pos.y))
            .rotation(s.rot)
            .build();
        let handle = self.bodies.insert(body);
        let collider = ColliderBuilder::cuboid(HALF_WIDTH, HALF_LENGTH)
            .friction(0.4)
            .restitution(0.1)
            .collision_groups(groups(G_CAR, G_CAR))
            .build();
        self.colliders
            .insert_with_parent(collider, handle, &mut self.bodies);
        self.remotes.insert(id, handle);
    }
    pub fn set_input(&mut self, id: PlayerId, input: Input) {
        if let Some(cur) = self.inputs.get_mut(&id) {
            *cur = Input {
                seq: input.seq,
                throttle: input.throttle.clamp(-1, 1),
                steer: input.steer.clamp(-1, 1),
                drift: input.drift,
            };
        }
    }
    pub fn set_remote(&mut self, id: PlayerId, pos: Vec2, rot: f32) {
        let Some(&h) = self.remotes.get(&id) else {
            return;
        };
        let body = &mut self.bodies[h];
        body.set_next_kinematic_translation(Vector::new(pos.x, pos.y));
        body.set_next_kinematic_rotation(Rotation::from_angle(rot));
    }
    pub fn remove_car(&mut self, id: PlayerId) {
        self.inputs.remove(&id);
        self.car_colliders.remove(&id);
        if let Some(handle) = self.cars.remove(&id) {
            self.bodies.remove(
                handle,
                &mut self.islands,
                &mut self.colliders,
                &mut self.impulse_joints,
                &mut self.multibody_joints,
                &mut self.soft_bodies,
                true,
            );
        }
    }
    pub fn remove_remote(&mut self, id: PlayerId) {
        if let Some(handle) = self.remotes.remove(&id) {
            self.bodies.remove(
                handle,
                &mut self.islands,
                &mut self.colliders,
                &mut self.impulse_joints,
                &mut self.multibody_joints,
                &mut self.soft_bodies,
                true,
            );
        }
    }

    pub fn step(&mut self) {
        let dt = self.integration.dt;
        let auth = self.authoritative;

        for (&id, &handle) in &self.cars {
            let touch = self.ground_touch(id);
            let surf = touch.surface.params();
            let input = self.inputs[&id];
            let ex = self.extras.get_mut(&id).unwrap();
            let body = &mut self.bodies[handle];
            let ch = self.car_colliders[&id];

            ex.invuln = ex.invuln.saturating_sub(1);
            ex.age += 1;

            let mut landed_vz = 0.;
            if ex.status != Status::Drowning && (ex.z > 0. || ex.vz > 0.) {
                ex.vz -= GRAVITY * dt;
                ex.z += ex.vz * dt;
                if ex.z <= 0. {
                    landed_vz = ex.vz;
                    ex.z = 0.;
                    ex.vz = 0.;
                }
            }
            let grounded = ex.z <= 0.;
            self.colliders[ch].set_collision_groups(match (ex.status, ex.z > CLEARANCE) {
                (Status::Drowning, _) => InteractionGroups::none(),
                (_, true) => groups(G_AIR, G_GATE),
                _ => groups(G_CAR, G_CAR | G_SOLID | G_ZONE | G_GATE),
            });

            if grounded && touch.water && ex.status != Status::Drowning {
                ex.status = Status::Drowning;
                ex.timer = DROWN_TICKS;
                ex.vz = landed_vz * 0.35 - 25.0;
            }

            if ex.status == Status::Drowning {
                ex.vz = (ex.vz - SINK_ACCEL * dt) * (-SINK_DRAG * dt).exp();
                ex.z = (ex.z + ex.vz * dt).max(-MAX_DEPTH);
                let v = body.linvel();
                let k = (-WATER_DRAG * dt).exp();
                body.set_linvel(Vector::new(v.x * k, v.y * k), true);
                body.set_angvel(body.angvel() * (-3.0 * dt).exp(), true);
                ex.timer = ex.timer.saturating_sub(1);
                if ex.timer == 0 && auth {
                    let (p, r) = ex.safe;
                    body.set_translation(Vector::new(p.x, p.y), true);
                    body.set_rotation(Rotation::from_angle(r), true);
                    body.set_linvel(Vector::new(r.sin() * 40.0, -r.cos() * 40.0), true);
                    body.set_angvel(0.0, true);
                    ex.z = 0.0;
                    ex.vz = 0.0;
                    ex.status = Status::Normal;
                    ex.invuln = 90;
                    ex.safe_next = ex.safe;
                }
                continue;
            }

            let mut launched = false;
            if touch.pad && grounded && ex.status == Status::Normal {
                ex.vz = JUMP_VZ;
                ex.z = 0.01;
                launched = true;
            }
            let airborne = ex.z > 0.0;
            let stunned = ex.status == Status::Spun;

            let rot = body.rotation().angle();
            let forward = Vector::new(rot.sin(), -rot.cos());
            let right = Vector::new(rot.cos(), rot.sin());
            let vel = body.linvel();
            let mut fwd = vel.dot(forward);
            let mut lat = vel.dot(right);

            let (lateral_grip, turn_rate) = if input.drift {
                (DRIFT_GRIP, DRIFT_TURN)
            } else {
                (LATERAL_GRIP, TURN_RATE)
            };

            if airborne {
                // momentum only
            } else if stunned {
                fwd *= (-2. * dt).exp();
            } else {
                let max_speed = MAX_SPEED * surf.max_speed;
                let max_reverse = MAX_REVERSE * surf.max_speed;

                let throttle = input.throttle as f32;
                if throttle != 0.0 {
                    let braking = fwd != 0.0 && throttle.signum() != fwd.signum();
                    let rate = if braking { BRAKE } else { ACCEL * surf.accel };
                    fwd += throttle * rate * dt;
                } else {
                    let decel = COAST_DECEL * dt;
                    fwd = if fwd.abs() <= decel {
                        0.0
                    } else {
                        fwd - fwd.signum() * decel
                    };
                }
                if fwd > max_speed {
                    fwd = (fwd - (COAST_DECEL + surf.extra_decel) * dt).max(max_speed);
                } else if fwd < -max_reverse {
                    fwd = (fwd + (COAST_DECEL + surf.extra_decel) * dt).min(-max_reverse);
                }
            }
            if launched {
                fwd = fwd.max(PAD_MIN_SPEED);
            }
            fwd = fwd.clamp(-MAX_REVERSE, MAX_SPEED);

            if !airborne {
                let grip = if stunned {
                    DRIFT_GRIP * 0.5
                } else {
                    lateral_grip
                };
                lat *= (-grip * surf.grip * dt).exp();
            }
            body.set_linvel(forward * fwd + right * lat, true);

            let target_ang = if stunned {
                let dir = if body.angvel() >= 0.0 { 1.0 } else { -1.0 };
                dir * SPIN_RATE * (ex.timer as f32 / STUN_TICKS as f32).max(0.25)
            } else {
                let control = if airborne { 0.35 } else { 1.0 };
                input.steer as f32 * turn_rate * (fwd / MIN_TURN_SPEED).clamp(-1.0, 1.0) * control
            };
            let ang = body.angvel();
            let blend = 1.0 - (-TURN_SMOOTH * dt).exp();
            body.set_angvel(ang + (target_ang - ang) * blend, true);

            if stunned {
                ex.timer = ex.timer.saturating_sub(1);
                if ex.timer == 0 {
                    ex.status = Status::Normal;
                }
            }

            if !airborne
                && !stunned
                && !touch.water
                && touch.surface == Surface::Road
                && ex.age % 30 == 0
            {
                ex.safe = ex.safe_next;
                let t = body.translation();
                ex.safe_next = (Vec2::new(t.x, t.y), rot);
            }
        }

        let pre: HashMap<PlayerId, Vec2> = self
            .cars
            .iter()
            .map(|(&id, &h)| {
                let v = self.bodies[h].linvel();
                (id, Vec2::new(v.x, v.y))
            })
            .collect();
        self.pipeline.step(
            Vector::new(0., 0.),
            &self.integration,
            &mut self.islands,
            &mut self.broad_phase,
            &mut self.narrow_phase,
            &mut self.bodies,
            &mut self.colliders,
            &mut self.impulse_joints,
            &mut self.multibody_joints,
            &mut self.soft_bodies,
            &mut self.ccd,
            &(),
            &(),
        );
        if auth {
            self.detect_hits(&pre);
        }

        #[cfg(feature = "debug-renderer")]
        {
            self.debug_backend.lines.clear();
            self.debug_render_pipeline.render(
                &mut self.debug_backend,
                &mut self.bodies,
                &mut self.colliders,
                &mut self.impulse_joints,
                &mut self.multibody_joints,
                &mut self.narrow_phase,
                &mut self.soft_bodies,
            );
        }
    }
    fn ground_touch(&self, id: PlayerId) -> Touch {
        let mut t = Touch {
            surface: Surface::Road,
            water: false,
            pad: false,
        };
        let Some(&ch) = self.car_colliders.get(&id) else {
            return t;
        };
        for (a, b, hit) in self.narrow_phase.intersection_pairs_with(ch) {
            if !hit {
                continue;
            }
            let ud = self.colliders[if a == ch { b } else { a }].user_data;
            if ud & CHECKPOINT_TAG != 0 {
                continue;
            }
            match ud {
                1 => t.surface = Surface::Grass,
                2 => t.water = true,
                3 => t.pad = true,
                _ => {}
            }
        }
        t
    }
    pub fn checkpoints_touching(&self, id: PlayerId) -> Vec<u32> {
        let Some(&ch) = self.car_colliders.get(&id) else {
            return vec![];
        };
        self.narrow_phase
            .intersection_pairs_with(ch)
            .filter(|(_, _, hit)| *hit)
            .filter_map(|(a, b, _)| {
                let other = if a == ch { b } else { a };
                let ud = self.colliders[other].user_data;
                (ud & CHECKPOINT_TAG != 0).then(|| (ud & 0xFFFF) as u32)
            })
            .collect()
    }

    fn detect_hits(&mut self, pre: &HashMap<PlayerId, Vec2>) {
        let ids: Vec<PlayerId> = self.cars.keys().copied().collect();
        for i in 0..ids.len() {
            for j in i + 1..ids.len() {
                let (a, b) = (ids[i], ids[j]);
                if !self.can_hit(a) || !self.can_hit(b) {
                    continue;
                }
                let touching = self
                    .narrow_phase
                    .contact_pair(self.car_colliders[&a], self.car_colliders[&b])
                    .is_some_and(|p| p.has_any_active_contact());
                if !touching {
                    continue;
                }

                let (pa, pb) = (self.pos(a), self.pos(b));
                let n = (pb - pa).normalize_or_zero();
                let (va, vb) = (pre[&a], pre[&b]);
                let closing = (va - vb).dot(n);
                if closing < HIT_MIN_SPEED {
                    continue;
                }

                let strength = (closing / MAX_SPEED).clamp(0.3, 1.0);
                let (sa, sb) = (va.length(), vb.length());
                let head_on = (sa - sb).abs() < 25.0;
                self.bump(a, n, strength, sa <= sb || head_on);
                self.bump(b, -n, strength, sb < sa || head_on);
            }
        }
    }
    fn can_hit(&self, id: PlayerId) -> bool {
        let e = &self.extras[&id];
        e.status == Status::Normal && e.invuln == 0 && e.z < CLEARANCE
    }
    fn bump(&mut self, id: PlayerId, toward: Vec2, strength: f32, victim: bool) {
        let body = &mut self.bodies[self.cars[&id]];
        let ex = self.extras.get_mut(&id).unwrap();
        ex.invuln = INVULN_TICKS;
        ex.vz = HIT_HOP * strength.max(0.5);
        ex.z = 0.01;
        if !victim {
            return;
        }
        let rot = body.rotation().angle();
        let right = Vec2::new(rot.cos(), rot.sin());
        let dir = if toward.dot(right) > 0.0 { -1.0 } else { 1.0 };
        ex.status = Status::Spun;
        ex.timer = (STUN_TICKS as f32 * (0.5 + 0.5 * strength)) as u16;
        body.set_angvel(dir * SPIN_RATE, true);
        let v = body.linvel();
        body.set_linvel(
            Vector::new(
                v.x - toward.x * 40.0 * strength,
                v.y - toward.y * 40.0 * strength,
            ),
            true,
        );
    }

    fn pos(&self, id: PlayerId) -> Vec2 {
        let body = &self.bodies[self.cars[&id]];
        let p = body.translation();
        Vec2::new(p.x, p.y)
    }

    pub fn state(&self, id: PlayerId) -> Option<CarState> {
        let handle = self.cars.get(&id)?;
        let body = self.bodies.get(*handle)?;
        let extra = self.extras.get(&id)?;
        Some(CarState {
            pos: Vec2 {
                x: body.translation().x,
                y: body.translation().y,
            },
            z: extra.z,
            rot: body.rotation().angle(),
            vel: Vec2 {
                x: body.linvel().x,
                y: body.linvel().y,
            },
            angular_vel: body.angvel(),
            ack: self.inputs.get(&id).map_or(0, |i| i.seq),
            vz: extra.vz,
            status: extra.status,
            timer: extra.timer,
            invuln: extra.invuln,
        })
    }
    pub fn states(&self) -> impl Iterator<Item = (PlayerId, CarState)> + '_ {
        self.cars
            .keys()
            .filter_map(|&id| self.state(id).map(|state| (id, state)))
    }
    pub fn set_state(&mut self, id: PlayerId, s: &CarState) {
        let Some(&h) = self.cars.get(&id) else { return };
        let Some(extra) = self.extras.get_mut(&id) else {
            return;
        };
        let body = &mut self.bodies[h];
        body.set_translation(Vector::new(s.pos.x, s.pos.y), true);
        body.set_rotation(Rotation::from_angle(s.rot), true);
        body.set_linvel(Vector::new(s.vel.x, s.vel.y), true);
        body.set_angvel(s.angular_vel, true);
        extra.z = s.z;
        extra.vz = s.vz;
        extra.status = s.status;
        extra.timer = s.timer;
        extra.invuln = s.invuln;
    }
}

//
// collision groups
//

const G_CAR: Group = Group::GROUP_1;
const G_AIR: Group = Group::GROUP_2;
const G_SOLID: Group = Group::GROUP_3;
const G_ZONE: Group = Group::GROUP_4;
const G_GATE: Group = Group::GROUP_5;

fn groups(memberships: Group, filter: Group) -> InteractionGroups {
    InteractionGroups::new(
        memberships,
        filter,
        rapier2d::geometry::InteractionTestMode::And,
    )
}

#[cfg(feature = "debug-renderer")]
pub struct LineCollector {
    pub lines: Vec<(Vector, Vector, DebugColor)>,
}
#[cfg(feature = "debug-renderer")]
impl DebugRenderBackend for LineCollector {
    fn draw_line(
        &mut self,
        _object: rapier2d::prelude::DebugRenderObject,
        a: Vector,
        b: Vector,
        color: DebugColor,
    ) {
        self.lines.push((a, b, color));
    }
}
