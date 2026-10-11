use parry2d::{
    math::{Pose, Vec2, Vector},
    query,
    shape::Cuboid,
};

pub mod coll;

const HALF_SIZE: Vec2 = Vec2::new(4., 7.5);

#[derive(Clone, Debug)]
pub struct CarState {
    pub pos: Vec2,
    pub rot: f32,
    pub vel: Vec2,
    pub mass: f32,
}

impl CarState {
    fn collider(&self) -> Cuboid {
        Cuboid::new(Vector::new(HALF_SIZE.x, HALF_SIZE.y))
    }
    fn pose(&self) -> Pose {
        Pose::new(self.pos, self.rot)
    }
}

pub fn resolve_car_collision(a: &mut CarState, b: &mut CarState) {
    let shape_a = a.collider();
    let shape_b = b.collider();
    let pose_a = a.pose();
    let pose_b = b.pose();

    let Ok(Some(contact)) = query::contact(&pose_a, &shape_a, &pose_b, &shape_b, 0.) else {
        return;
    };

    let normal = Vec2::new(contact.normal1.x, contact.normal1.y);
}

pub struct CarInput {
    pub throttle: f32,
    pub steer: f32,
}

pub struct PhysicsWorld {
    pub cars: Vec<CarState>,
}
