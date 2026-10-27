use raylib::math::Vector2;

#[derive(Default)]
pub struct Camera {
    p: Vector2,
    a: f32,
    vel: Vector2,
    av: f32,
    init: bool,

    last_target: Vector2,
    target_vel: Vector2,

    shake: f32,
    shake_t: f32,
    off: Vector2,
    boost_prev: u8,
}

const POS_SMOOTH: f32 = 0.10;
const ROT_SMOOTH: f32 = 0.35;
const MAX_ROT_SPEED: f32 = 240.0;

const LOOK_TIME: f32 = 0.5;
const MAX_LOOK: f32 = 70.0;
const VEL_SMOOTH: f32 = 0.30;
const MAX_PLAUSIBLE_SPEED: f32 = 3000.0;

const MAX_SHAKE_PX: f32 = 4.0;
const SHAKE_DECAY: f32 = 1.6;
const SHAKE_HZ: f32 = 40.0;

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
fn hash(n: i32) -> f32 {
    let mut x = n as u32;
    x = x.wrapping_mul(0x9E3779B1) ^ (x >> 15);
    x = x.wrapping_mul(0x85EBCA6B);
    x ^= x >> 13;
    (x & 0xFFFF) as f32 / 32767.5 - 1.0
}

impl Camera {
    pub fn update(&mut self, target: Vector2, target_a: f32, raw_dt: f32) {
        let dt = raw_dt.min(1.0 / 20.0);
        if !self.init {
            self.p = target;
            self.a = target_a;
            self.last_target = target;
            self.init = true;
            return;
        }
        self.fx_step(dt);

        if raw_dt > 1e-5 {
            let raw = Vector2::new(
                (target.x - self.last_target.x) / raw_dt,
                (target.y - self.last_target.y) / raw_dt,
            );
            if raw.length() > MAX_PLAUSIBLE_SPEED {
                self.target_vel = Vector2::zero();
            } else {
                let k = 1.0 - (-raw_dt / VEL_SMOOTH).exp();
                self.target_vel.x += (raw.x - self.target_vel.x) * k;
                self.target_vel.y += (raw.y - self.target_vel.y) * k;
            }
        }
        self.last_target = target;

        let speed = self.target_vel.length();
        let look = if speed > 1e-3 {
            let dist = MAX_LOOK * (speed * LOOK_TIME / MAX_LOOK).tanh();
            Vector2::new(
                self.target_vel.x / speed * dist,
                self.target_vel.y / speed * dist,
            )
        } else {
            Vector2::zero()
        };
        let aim = Vector2::new(target.x + look.x, target.y + look.y);

        self.p.x = smooth_damp(self.p.x, aim.x, &mut self.vel.x, POS_SMOOTH, dt);
        self.p.y = smooth_damp(self.p.y, aim.y, &mut self.vel.y, POS_SMOOTH, dt);

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
        let rx = tx * c - ty * s;
        let ry = tx * s + ty * c;
        (
            Vector2 {
                x: rx + 160.0 + self.off.x,
                y: ry + 90.5 + self.off.y,
            },
            a - self.a,
        )
    }
    pub fn inverse(&self) -> (Vector2, [f32; 4]) {
        let (o, _) = self.apply(Vector2::zero(), 0.);
        let ex = self.apply(Vector2::new(1., 0.), 0.).0 - o;
        let ey = self.apply(Vector2::new(0., 1.), 0.).0 - o;
        let det = ex.x * ey.y - ex.y * ey.x;
        (o, [ey.y / det, -ey.x / det, -ex.y / det, ex.x / det])
    }

    pub fn add_shake(&mut self, amt: f32) {
        self.shake = (self.shake + amt).min(1.0);
    }
    pub fn set_boost(&mut self, boost: u8) {
        if boost > 0 && self.boost_prev == 0 {
            self.add_shake(0.5);
        }
        self.boost_prev = boost;
    }
    fn fx_step(&mut self, dt: f32) {
        self.shake = (self.shake - SHAKE_DECAY * dt).max(0.0);
        self.shake_t += dt;
        let step = (self.shake_t * SHAKE_HZ) as i32;
        let amp = self.shake * MAX_SHAKE_PX;
        self.off = Vector2::new(
            (hash(step * 2) * amp).round(),
            (hash(step * 2 + 1) * amp).round(),
        );
    }
}
