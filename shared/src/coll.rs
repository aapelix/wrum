use std::ops::{Add, AddAssign, Mul, Neg, Sub, SubAssign};

use rkyv::{Archive, Deserialize, Serialize};

#[derive(Archive, Serialize, Deserialize, Debug, Default, Clone, Copy)]
pub struct Vec2 {
    pub x: f32,
    pub y: f32,
}
impl Vec2 {
    pub fn new(x: f32, y: f32) -> Vec2 {
        Vec2 { x, y }
    }
    pub fn dot(&self, v: Vec2) -> f32 {
        self.x * v.x + self.y * v.y
    }
}
impl Neg for Vec2 {
    type Output = Vec2;
    fn neg(self) -> Self {
        Vec2 {
            x: -self.x,
            y: -self.y,
        }
    }
}
impl Mul<f32> for Vec2 {
    type Output = Vec2;
    fn mul(self, value: f32) -> Self {
        Vec2 {
            x: self.x * value,
            y: self.y * value,
        }
    }
}
impl Sub for Vec2 {
    type Output = Vec2;
    fn sub(self, rhs: Self) -> Self::Output {
        Vec2 {
            x: self.x - rhs.x,
            y: self.y - rhs.y,
        }
    }
}
impl Sub<f32> for Vec2 {
    type Output = Vec2;
    fn sub(self, value: f32) -> Self {
        Vec2 {
            x: self.x - value,
            y: self.y - value,
        }
    }
}
impl SubAssign for Vec2 {
    fn sub_assign(&mut self, v: Vec2) {
        *self = *self - v;
    }
}
impl SubAssign<f32> for Vec2 {
    fn sub_assign(&mut self, value: f32) {
        *self = *self - value;
    }
}
impl Add for Vec2 {
    type Output = Vec2;
    fn add(self, rhs: Self) -> Self::Output {
        Vec2 {
            x: self.x + rhs.x,
            y: self.y + rhs.y,
        }
    }
}
impl Add<f32> for Vec2 {
    type Output = Vec2;
    fn add(self, value: f32) -> Self {
        Vec2 {
            x: self.x + value,
            y: self.y + value,
        }
    }
}
impl AddAssign for Vec2 {
    fn add_assign(&mut self, v: Vec2) {
        *self = *self + v;
    }
}
impl AddAssign<f32> for Vec2 {
    fn add_assign(&mut self, value: f32) {
        *self = *self + value;
    }
}

#[derive(Clone, Copy)]
pub struct Obb {
    pub pos: Vec2,
    pub halfs: Vec2,
    pub a: f32,
}

pub struct CollisionBody {
    pub box_: Obb,
    pub vel: Vec2,
    pub inv_mass: f32,
}

fn obb_axis(b: &Obb, which: usize) -> Vec2 {
    let (sin, cos) = b.a.sin_cos();

    if which == 0 {
        Vec2::new(cos, sin)
    } else {
        Vec2::new(-sin, cos)
    }
}
fn obb_project_radius(b: &Obb, axis: Vec2) -> f32 {
    let ax0 = obb_axis(b, 0);
    let ax1 = obb_axis(b, 1);

    ax0.dot(axis).abs() * b.halfs.x + ax1.dot(axis).abs() * b.halfs.y
}
pub fn obb_overlap(a: &Obb, b: &Obb) -> Option<(Vec2, f32)> {
    let axes = [
        obb_axis(a, 0),
        obb_axis(a, 1),
        obb_axis(b, 0),
        obb_axis(b, 1),
    ];

    let d = Vec2::new(b.pos.x - a.pos.x, b.pos.y - a.pos.y);

    let mut best_depth = f32::MAX;
    let mut best_axis = Vec2::new(1.0, 0.0);

    for axis in axes {
        let center_dist = d.dot(axis).abs();

        let overlap = obb_project_radius(a, axis) + obb_project_radius(b, axis) - center_dist;

        if overlap <= 0.0 {
            return None;
        }

        if overlap < best_depth {
            best_depth = overlap;
            best_axis = axis;
        }
    }

    if d.dot(best_axis) < 0.0 {
        best_axis = -best_axis;
    }

    Some((best_axis, best_depth))
}
pub fn resolve_collision(a: &mut CollisionBody, b: &mut CollisionBody, restitution: f32) {
    let Some((normal, depth)) = obb_overlap(&a.box_, &b.box_) else {
        return;
    };

    let total_inv_mass = a.inv_mass + b.inv_mass;

    if total_inv_mass <= 0.0 {
        return;
    }

    let slop = 0.5;
    let percent = 0.8;

    let corr = (depth - slop).max(0.0) * percent / total_inv_mass;

    let correction_a = normal * (corr * a.inv_mass);
    let correction_b = normal * (corr * b.inv_mass);

    a.box_.pos.x -= correction_a.x;
    a.box_.pos.y -= correction_a.y;

    b.box_.pos.x += correction_b.x;
    b.box_.pos.y += correction_b.y;

    let rel = b.vel - a.vel;
    let vn = rel.dot(normal);

    if vn > 0.0 {
        return;
    }

    let j = -(1.0 + restitution) * vn / total_inv_mass;
    let impulse = normal * j;

    a.vel -= impulse * a.inv_mass;
    b.vel += impulse * b.inv_mass;
}
pub fn obb_from_segment(p0: Vec2, p1: Vec2, thickness: f32) -> Obb {
    let dx = p1.x - p0.x;
    let dy = p1.y - p0.y;

    let len = (dx * dx + dy * dy).sqrt();

    let a = (-dx).atan2(dy);

    Obb {
        pos: Vec2 {
            x: (p0.x + p1.x) * 0.5,
            y: (p0.y + p1.y) * 0.5,
        },
        halfs: Vec2 {
            x: thickness * 0.5,
            y: len * 0.5,
        },
        a,
    }
}
