use std::collections::HashMap;

pub use glam::Vec2;
use rapier2d::math::Rotation;
pub use rapier2d::{
    dynamics::{
        CCDSolver, ImpulseJointSet, IntegrationParameters, IslandManager, MultibodyJointSet,
        RigidBodyBuilder, RigidBodyHandle, RigidBodySet, SoftBodySet,
    },
    geometry::{BroadPhaseBvh, ColliderBuilder, ColliderSet, NarrowPhase},
    math::Vector,
    pipeline::PhysicsPipeline,
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

const HALF_WIDTH: f32 = 4.;
const HALF_LENGTH: f32 = 7.5;

const CAR_MASS: f32 = 1.0;
const MAX_SPEED: f32 = 180.0;
const MAX_REVERSE: f32 = 80.0;
const ACCEL: f32 = 220.0;
const BRAKE: f32 = 450.0;
const COAST_DECEL: f32 = 120.0;
const LATERAL_GRIP: f32 = 16.0;
const DRIFT_GRIP: f32 = 5.0;
const TURN_RATE: f32 = 2.8;
const DRIFT_TURN: f32 = TURN_RATE * 1.3;
const TURN_SMOOTH: f32 = 14.0;
const MIN_TURN_SPEED: f32 = 25.0;

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
    remotes: HashMap<PlayerId, RigidBodyHandle>,
    inputs: HashMap<PlayerId, Input>,
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
            cars: HashMap::new(),
            remotes: HashMap::new(),
            inputs: HashMap::new(),
        }
    }
}

impl PhysicsWorld {
    pub fn load_level(&mut self, level: &Level) {
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
        self.colliders
            .insert_with_parent(collider, handle, &mut self.bodies);

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
            let body = &mut self.bodies[handle];

            // body.reset_forces(true);
            // body.reset_torques(true);

            let rot = body.rotation().angle();
            let forward = Vector::new(rot.sin(), -rot.cos());
            let right = Vector::new(rot.cos(), rot.sin());

            let vel = body.linvel();
            let mut fwd = vel.dot(forward);
            let mut lat = vel.dot(right);

            let throttle = input.throttle as f32;
            if throttle != 0.0 {
                let braking = fwd != 0.0 && throttle.signum() != fwd.signum();
                let rate = if braking { BRAKE } else { ACCEL };
                fwd += throttle * rate * dt;
            } else {
                let decel = COAST_DECEL * dt;
                fwd = if fwd.abs() <= decel {
                    0.0
                } else {
                    fwd - fwd.signum() * decel
                };
            }
            fwd = fwd.clamp(-MAX_REVERSE, MAX_SPEED);

            let (lateral_grip, turn_rate) = if input.drift {
                (DRIFT_GRIP, DRIFT_TURN)
            } else {
                (LATERAL_GRIP, TURN_RATE)
            };

            lat *= (-lateral_grip * dt).exp();

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
