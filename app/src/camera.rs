use raylib::math::Vector2;

#[derive(Default)]
pub struct Camera {
    p: Vector2,
    a: f32,
    vel: Vector2,
    av: f32,
    init: bool,
}

const POS_SMOOTH: f32 = 0.10;
const ROT_SMOOTH: f32 = 0.35;
const MAX_ROT_SPEED: f32 = 240.0;

fn angle_diff(target: f32, current: f32) -> f32 {
    (target - current + 180.0).rem_euclid(360.0) - 180.0
}
fn smooth_damp(cur: f32, target: f32, vel: &mut f32, smooth_time: f32, dt: f32) -> f32 {
    let omega = 2.0 / smooth_time.max(1e-4);
    let x = omega * dt;
    let e = 1.0 / (1.0 + x + 0.48 * x * x + 0.235 * x * x * x);
    let change = cur - target;
    let temp = (*vel + omega * change) * dt;
    *vel = (*vel - omega * temp) * e;
    target + (change + temp) * e
}

impl Camera {
    pub fn update(&mut self, target: Vector2, target_a: f32, dt: f32) {
        let dt = dt.min(1.0 / 20.0);
        if !self.init {
            self.p = target;
            self.a = target_a;
            self.init = true;
            return;
        }

        self.p.x = smooth_damp(self.p.x, target.x, &mut self.vel.x, POS_SMOOTH, dt);
        self.p.y = smooth_damp(self.p.y, target.y, &mut self.vel.y, POS_SMOOTH, dt);

        let unwrapped = self.a + angle_diff(target_a, self.a);
        self.a = smooth_damp(self.a, unwrapped, &mut self.av, ROT_SMOOTH, dt);
        self.av = self.av.clamp(-MAX_ROT_SPEED, MAX_ROT_SPEED);
        self.a = self.a.rem_euclid(360.0);
    }
    pub fn apply(&self, p: Vector2, a: f32) -> (Vector2, f32) {
        let tx = p.x - self.p.x;
        let ty = p.y - self.p.y;
        let rad = -self.a.to_radians();
        let (s, c) = rad.sin_cos();
        (
            Vector2 {
                x: tx * c - ty * s + 160.0,
                y: tx * s + ty * c + 90.5,
            },
            a - self.a,
        )
    }
}
