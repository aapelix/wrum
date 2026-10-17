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
use rapier2d::{geometry::ColliderHandle, math::Rotation};
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
}
struct SurfaceParams {
    max_speed: f32,
    accel: f32,
    grip: f32,
    extra_decel: f32,
}
impl Surface {
    fn from_u128(v: u128) -> Self {
        if v == 1 {
            Surface::Grass
        } else {
            Surface::Road
        }
    }
    fn priority(self) -> u8 {
        self as u8
    }
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
        }
    }
}

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

#[derive(Archive, Serialize, Deserialize, Clone, Copy, Debug, Default)]
pub struct CarState {
    pub pos: Vec2,
    pub rot: f32,
    pub vel: Vec2,
    pub angular_vel: f32,
    pub ack: u32,
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
            .build();
        let ch = self
            .colliders
            .insert_with_parent(collider, handle, &mut self.bodies);

        self.car_colliders.insert(id, ch);
        self.cars.insert(id, handle);
        self.inputs.insert(id, Input::default());
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

        for (&id, &handle) in &self.cars {
            let input = self.inputs[&id];
            let surf = self.surface_under(id).params();
            let body = &mut self.bodies[handle];

            // body.reset_forces(true);
            // body.reset_torques(true);

            let rot = body.rotation().angle();
            let forward = Vector::new(rot.sin(), -rot.cos());
            let right = Vector::new(rot.cos(), rot.sin());

            let vel = body.linvel();
            let mut fwd = vel.dot(forward);
            let mut lat = vel.dot(right);

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
            fwd = fwd.clamp(-MAX_REVERSE, MAX_SPEED);

            let (lateral_grip, turn_rate) = if input.drift {
                (DRIFT_GRIP, DRIFT_TURN)
            } else {
                (LATERAL_GRIP, TURN_RATE)
            };

            lat *= (-lateral_grip * surf.grip * dt).exp();

            body.set_linvel(forward * fwd + right * lat, true);

            let speed_factor = (fwd / MIN_TURN_SPEED).clamp(-1.0, 1.0);
            let target_ang = input.steer as f32 * turn_rate * speed_factor;
            let ang = body.angvel();
            let blend = 1.0 - (-TURN_SMOOTH * dt).exp();
            body.set_angvel(ang + (target_ang - ang) * blend, true);
        }

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
    fn surface_under(&self, id: PlayerId) -> Surface {
        let Some(&ch) = self.car_colliders.get(&id) else {
            return Surface::Road;
        };
        let mut worst = Surface::Road;
        for (a, b, intersecting) in self.narrow_phase.intersection_pairs_with(ch) {
            if !intersecting {
                continue;
            }
            let other = if a == ch { b } else { a };
            let s = Surface::from_u128(self.colliders[other].user_data);
            if s.priority() > worst.priority() {
                worst = s;
            }
        }
        worst
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

    pub fn state(&self, id: PlayerId) -> Option<CarState> {
        let handle = self.cars.get(&id)?;
        let body = self.bodies.get(*handle)?;
        Some(CarState {
            pos: Vec2 {
                x: body.translation().x,
                y: body.translation().y,
            },
            rot: body.rotation().angle(),
            vel: Vec2 {
                x: body.linvel().x,
                y: body.linvel().y,
            },
            angular_vel: body.angvel(),
            ack: self.inputs.get(&id).map_or(0, |i| i.seq),
        })
    }
    pub fn states(&self) -> impl Iterator<Item = (PlayerId, CarState)> + '_ {
        self.cars
            .keys()
            .filter_map(|&id| self.state(id).map(|state| (id, state)))
    }
    pub fn set_state(&mut self, id: PlayerId, s: &CarState) {
        let Some(&h) = self.cars.get(&id) else { return };
        let body = &mut self.bodies[h];
        body.set_translation(Vector::new(s.pos.x, s.pos.y), true);
        body.set_rotation(Rotation::from_angle(s.rot), true);
        body.set_linvel(Vector::new(s.vel.x, s.vel.y), true);
        body.set_angvel(s.angular_vel, true);
    }
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
