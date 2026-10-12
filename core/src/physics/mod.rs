use std::collections::HashMap;

use glam::Vec2;
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

use crate::PlayerId;

#[derive(Archive, Serialize, Deserialize, Debug, Clone, Copy, Default)]
pub struct Input {
    pub seq: u32,
    pub throttle: i8,
    pub steer: i8,
}

const HALF_WIDTH: f32 = 4.;
const HALF_LENGTH: f32 = 7.5;

const CAR_MASS: f32 = 1.0;
const ACC: f32 = 500.;
const LATERAL_GRIP: f32 = 12.0;
const DRAG: f32 = 0.8;
const TURN_TORQUE: f32 = 100.0;
const MAX_SPEED: f32 = 250.0;

#[derive(Archive, Serialize, Deserialize, Clone, Copy, Debug, Default)]
pub struct CarState {
    pub pos: Vec2,
    pub rot: f32,
    pub vel: Vec2,
    pub angular_vel: f32,
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
    inputs: HashMap<PlayerId, Input>,
}

impl Default for PhysicsWorld {
    fn default() -> Self {
        let mut integration = IntegrationParameters::default();
        integration.dt = 1.0 / 60.;

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
            inputs: HashMap::new(),
        }
    }
}

impl PhysicsWorld {
    pub fn spawn_car(&mut self, id: PlayerId, pos: Vec2) {
        let body = RigidBodyBuilder::dynamic()
            .translation(Vector::new(pos.x, pos.y))
            .additional_mass(CAR_MASS)
            .linear_damping(0.)
            .angular_damping(0.)
            .can_sleep(false)
            .ccd_enabled(true)
            .build();
        let handle = self.bodies.insert(body);
        let collider = ColliderBuilder::cuboid(HALF_WIDTH, HALF_LENGTH)
            .friction(0.4)
            .restitution(0.1)
            .build();
        self.colliders
            .insert_with_parent(collider, handle, &mut self.bodies);

        self.cars.insert(id, handle);
        self.inputs.insert(id, Input::default());
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
    pub fn set_input(&mut self, id: PlayerId, input: Input) {
        if let Some(current) = self.inputs.get_mut(&id) {
            current.throttle = input.throttle.clamp(-1, 1);
            current.steer = input.steer.clamp(-1, 1);
        }
    }

    pub fn step(&mut self) {
        for (&id, &handle) in &self.cars {
            let input = self.inputs[&id];
            let body = &mut self.bodies[handle];

            body.reset_forces(true);
            body.reset_torques(true);

            let rot = body.rotation().angle();
            let forward = Vector::new(-rot.sin(), rot.cos());
            let right = Vector::new(forward.y, -forward.x);

            let vel = body.linvel();
            let forward_speed = vel.dot(forward);
            let lateral_speed = vel.dot(right);

            body.add_force(forward * (input.throttle as f32 * ACC), true);
            body.add_force(right * (-lateral_speed * LATERAL_GRIP), true);
            body.add_force(-vel * DRAG, true);

            let speed_factor = (forward_speed.abs() / 80.).clamp(0., 1.);

            body.add_torque(input.steer as f32 * TURN_TORQUE * speed_factor, true);

            let speed = vel.length();
            if speed > MAX_SPEED {
                body.set_linvel(vel * (MAX_SPEED / speed), true);
            }
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
        })
    }
    pub fn states(&self) -> impl Iterator<Item = (PlayerId, CarState)> + '_ {
        self.cars
            .keys()
            .filter_map(|&id| self.state(id).map(|state| (id, state)))
    }
}

// impl CarState {
//     fn collider(&self) -> Cuboid {
//         Cuboid::new(Vector::new(HALF_SIZE.x, HALF_SIZE.y))
//     }
//     fn pose(&self) -> Pose {
//         Pose::new(self.pos, self.rot)
//     }
// }
