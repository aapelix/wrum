use raylib::math::Vector2;

#[derive(Default)]
pub struct Camera {
    p: Vector2,
    a: f32,
}

fn angle_diff(target: f32, current: f32) -> f32 {
    (target - current + 180.0).rem_euclid(360.0) - 180.0
}

impl Camera {
    pub fn update(&mut self, target: Vector2, target_a: f32, dt: f32) {
        let t = 1.0 - (-10.0 * dt).exp();
        self.p.x += (target.x - self.p.x) * t;
        self.p.y += (target.y - self.p.y) * t;
        self.a += angle_diff(target_a, self.a) * t;
    }
    pub fn apply(&self, p: Vector2, a: f32) -> (Vector2, f32) {
        let tx = p.x - self.p.x;
        let ty = p.y - self.p.y;

        let rad = -(self.a).to_radians();
        let s = rad.sin();
        let c = rad.cos();

        let rx = tx * c - ty * s;
        let ry = tx * s + ty * c;

        (
            Vector2 {
                x: rx + 160.0,
                y: ry + 90.0,
            },
            a - self.a,
        )
    }
}
