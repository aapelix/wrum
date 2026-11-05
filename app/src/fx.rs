use std::collections::HashMap;

use raylib::{
    RaylibHandle,
    color::Color,
    consts::BlendMode,
    drawing::{
        RaylibBlendModeExt, RaylibDraw, RaylibShaderModeExt, RaylibTextureMode,
        RaylibTextureModeExt,
    },
    ffi::{rlDisableBackfaceCulling, rlEnableBackfaceCulling},
    math::Vector2,
    shaders::{RaylibShader, Shader},
    texture::{RaylibRenderTexture2D, RaylibTexture2D, RenderTexture2D},
};
use wrum_core::{
    PlayerId,
    level::Level,
    sim::{Status, V2},
};

use crate::{
    GLSL_VERSION,
    camera::Camera,
    car::{Car, TIRE_POSITIONS},
    ctx::Context,
};

const W_FOAM: Color = Color::new(237, 247, 255, 255);
const W_LIGHT: Color = Color::new(89, 166, 230, 255);
const W_DARK: Color = Color::new(20, 51, 115, 255);

const SPLASH_G: f32 = 320.0;
const IMPACT_MAX: f32 = 140.0;

struct Rng(u32);
impl Rng {
    fn next(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 17;
        self.0 ^= self.0 << 5;
        (self.0 >> 8) as f32 / 16_777_216.0
    }
    fn range(&mut self, a: f32, b: f32) -> f32 {
        a + (b - a) * self.next()
    }
}

fn smooth(a: f32, b: f32, x: f32) -> f32 {
    let t = ((x - a) / (b - a)).clamp(0., 1.);
    t * t * (3. - 2. * t)
}

#[derive(Clone, Copy)]
enum Kind {
    Trail,
    Spark(u8),
    Flame(u8),
}
struct Bit {
    kind: Kind,
    p: Vector2,
    v: Vector2,
    dir: Vector2,
    z: f32,
    vz: f32,
    age: f32,
    life: f32,
    len: f32,
}
struct Puff {
    p: Vector2,
    v: Vector2,
    z: f32,
    vz: f32,
    age: f32,
    life: f32,
    r0: f32,
    r1: f32,
    a: f32,
    tone: u8,
}

struct Droplet {
    p: Vector2,
    v: Vector2,
    z: f32,
    vz: f32,
    big: bool,
}
struct Ripple {
    p: Vector2,
    age: f32,
    life: f32,
    r1: f32,
}

#[derive(Default)]
struct Track {
    prev_p: Option<Vector2>,
    vel: Vector2,
    prev_fwd: f32,
    decel: f32,
    last: [Option<Vector2>; 4],
    smoke: [f32; 4],
    peak_z: f32,
    airborne: bool,
    land: f32,
    streak: f32,
    spark: [f32; 2],
    flame: f32,
    boost_prev: u8,
    prev_z: f32,
    fall: f32,
    was_wet: bool,
    tier_prev: u8,
    charge_prev: u16,
    perfect_prev: bool,
    dsmoke: [f32; 2],
    soot: f32,
}

pub struct Fx {
    pub marks: RenderTexture2D,
    origin: Vector2,
    pub size: Vector2,
    pending: Vec<(Vector2, Vector2, u8)>,
    puffs: Vec<Puff>,
    tracks: HashMap<PlayerId, Track>,
    rng: Rng,
    bits: Vec<Bit>,
    droplets: Vec<Droplet>,
    ripples: Vec<Ripple>,

    shader: Shader,
    loc_origin: i32,
    loc_inv: i32,
    loc_marks: i32,
    loc_morigin: i32,
    loc_msize: i32,
}

impl Fx {
    pub fn new(ctx: &mut Context, origin: Vector2, size: Vector2) -> Self {
        let mut marks = ctx
            .rl
            .load_render_texture(&ctx.thread, size.x.ceil() as u32, size.y.ceil() as u32)
            .unwrap();
        {
            let mut tm = ctx.rl.begin_texture_mode(&ctx.thread, &mut marks);
            tm.clear_background(Color::BLACK);
        }
        marks.texture().set_texture_filter(
            &ctx.thread,
            raylib::ffi::TextureFilter::TEXTURE_FILTER_BILINEAR,
        );

        let dir = format!("assets/shaders/glsl{}", GLSL_VERSION);
        let shader = ctx.rl.load_shader(
            &ctx.thread,
            Some(&format!("{dir}/water.vs")),
            Some(&format!("{dir}/marks.fs")),
        );
        Self {
            loc_origin: shader.get_shader_location("uOrigin"),
            loc_inv: shader.get_shader_location("uInv"),
            loc_marks: shader.get_shader_location("uMarks"),
            loc_morigin: shader.get_shader_location("uMarkOrigin"),
            loc_msize: shader.get_shader_location("uMarkSize"),
            shader,
            marks,
            origin,
            size,
            pending: Vec::new(),
            puffs: Vec::new(),
            tracks: HashMap::new(),
            rng: Rng(0x9E3779B9),
            bits: Vec::new(),
            droplets: Vec::new(),
            ripples: Vec::new(),
        }
    }

    // ---------------------------------------------------------------- update

    pub fn update(
        &mut self,
        dt: f32,
        cars: &HashMap<PlayerId, Car>,
        level: &Level,
        water: &dyn Fn(Vector2) -> bool,
    ) {
        if dt <= 0.0 {
            return;
        }
        self.tracks.retain(|id, _| cars.contains_key(id));

        for (id, car) in cars {
            let grass = level.on_grass(V2::new(car.p.x, car.p.y));
            let mut tr = self.tracks.remove(id).unwrap_or_default();
            self.step_car(&mut tr, car, dt, grass, water);
            self.tracks.insert(*id, tr);
        }

        for p in &mut self.puffs {
            p.age += dt;
            p.p += p.v * dt;
            p.v *= (-2.2 * dt).exp();
            p.z += p.vz * dt;
            p.vz *= (-1.2 * dt).exp();
        }
        self.puffs.retain(|p| p.age < p.life);

        for b in &mut self.bits {
            b.age += dt;
            b.p += b.v * dt;
            match b.kind {
                Kind::Trail => {
                    b.v *= (-4.0 * dt).exp();
                }
                Kind::Spark(_) => {
                    b.v *= (-2.5 * dt).exp();
                    b.vz -= 220.0 * dt;
                    b.z += b.vz * dt;
                    if b.z < 0.0 {
                        b.z = 0.0;
                        b.vz *= -0.3;
                    }
                }
                Kind::Flame(_) => {
                    b.v *= (-3.0 * dt).exp();
                    let n = Vector2::new(-b.dir.y, b.dir.x);
                    let sway = (b.age * 28.0 + b.vz * 40.0).sin() * 45.0 * (b.age / b.life);
                    b.p += n * sway * dt;
                }
            }
        }
        self.bits.retain(|b| b.age < b.life);

        for d in &mut self.droplets {
            d.p += d.v * dt;
            d.v *= (-1.0 * dt).exp();
            d.vz -= SPLASH_G * dt;
            d.z += d.vz * dt;
        }
        let (ripples, rng) = (&mut self.ripples, &mut self.rng);
        self.droplets.retain(|d| {
            if d.z >= 0.0 || d.vz > 0.0 {
                return true;
            }
            if water(d.p) && ripples.len() < 80 && rng.next() < if d.big { 0.7 } else { 0.3 } {
                ripples.push(Ripple {
                    p: d.p,
                    age: 0.0,
                    life: 0.35,
                    r1: if d.big { 4.0 } else { 3.0 },
                });
            }
            false
        });
        for r in &mut self.ripples {
            r.age += dt;
        }
        self.ripples.retain(|r| r.age < r.life);
    }
    pub fn clear(&mut self, ctx: &mut Context) {
        {
            let mut tm = ctx.rl.begin_texture_mode(&ctx.thread, &mut self.marks);
            tm.clear_background(Color::BLACK);
        }

        self.pending.clear();
        self.tracks.clear();
        self.bits.clear();
        self.droplets.clear();
        self.ripples.clear();
    }

    fn step_car(
        &mut self,
        tr: &mut Track,
        car: &Car,
        dt: f32,
        grass: bool,
        water: &dyn Fn(Vector2) -> bool,
    ) {
        let Some(prev) = tr.prev_p.replace(car.p) else {
            return;
        };
        let sub = car.p - prev;
        let raw = Vector2::new(sub.x / dt, sub.y / dt);
        if raw.length() > 450.0 {
            tr.last = [None; 4];
            tr.vel = Vector2::zero();
            return;
        }
        tr.vel += (raw - tr.vel) * (1.0 - (-30.0 * dt).exp());

        let (s, c) = car.a.to_radians().sin_cos();
        let (fwd_v, right_v) = (Vector2::new(s, -c), Vector2::new(c, s));
        let fwd = tr.vel.dot(fwd_v);
        let slip = tr.vel.dot(right_v).abs();
        let decel = (tr.prev_fwd - fwd) / dt;
        tr.prev_fwd = fwd;
        tr.decel += (decel - tr.decel) * (1.0 - (-20.0 * dt).exp());

        let up = car.z > 0.5;
        let vz = (car.z - tr.prev_z) / dt;
        tr.prev_z = car.z;
        if up || tr.airborne {
            tr.fall = tr.fall.max(-vz);
        }
        if up {
            tr.peak_z = tr.peak_z.max(car.z);
        }
        let wet = water(car.p);
        let mut landed = false;
        if tr.airborne && !up {
            if tr.peak_z > 6.0 {
                tr.land = (tr.peak_z / 25.0).clamp(0.3, 1.0);
                if wet {
                    let s = (tr.fall / IMPACT_MAX).max(tr.peak_z / 40.0).clamp(0.2, 1.0);
                    self.splash(car.p, s);
                    landed = true;
                } else {
                    self.landing_burst(car.p, tr.land, grass);
                }
            }
            tr.peak_z = 0.0;
            tr.fall = 0.0;
        }
        tr.airborne = up;

        if wet && !tr.was_wet && !landed && !up {
            let k = smooth(60.0, 260.0, tr.vel.length());
            if k > 0.1 {
                self.splash(car.p, k * 0.6);
            }
        }
        tr.was_wet = wet;

        let grounded = !up && car.z > -0.01 && car.status != Status::Drowning;

        let slide = smooth(22.0, 65.0, slip);
        let hard = if fwd > 30.0 {
            smooth(200.0, 380.0, tr.decel)
        } else {
            0.0
        };
        let spun = if car.status == Status::Spun { 0.9 } else { 0.0 };
        tr.land = (tr.land - dt * 4.0).max(0.0);
        let boost_k = if fwd.abs() > 60.0 { tr.land * 0.8 } else { 0.0 };

        let rear = slide.max(hard).max(spun).max(boost_k);
        let front = (slide * smooth(40.0, 90.0, slip))
            .max(hard * 0.7)
            .max(spun * 0.8)
            * 0.6;

        let speed = tr.vel.length();
        let dir = if speed > 1.0 {
            tr.vel * (1.0 / speed)
        } else {
            fwd_v
        };
        let drifting = car.drift_dir != 0 && grounded;
        let frac = (car.charge as f32 / 360.0).min(1.0);

        let sk = smooth(150.0, 260.0, speed);
        let sk = if car.draft { sk.max(0.75) } else { sk };
        if grounded && !grass && sk > 0.0 {
            tr.streak += (5.0 + 48.0 * sk) * dt;
            while tr.streak >= 1.0 && self.bits.len() < 900 {
                tr.streak -= 1.0;
                for _ in 0..2 {
                    let r = &mut self.rng;
                    let back = dir * (-speed * dt * r.next());
                    self.bits.push(Bit {
                        kind: Kind::Trail,
                        p: car.p - fwd_v * 4.0 + right_v * r.range(-4.0, 4.0) + back,
                        v: right_v * r.range(-6.0, 6.0) + dir * (-speed * 0.08),
                        dir,
                        z: 0.0,
                        vz: r.next(),
                        age: 0.0,
                        life: r.range(0.2, 0.45),
                        len: 3.5 + 7.0 * sk * r.range(0.6, 1.3),
                    });
                }
            }
        } else {
            tr.streak = 0.0;
        }

        if car.boost > 0 && car.status != Status::Drowning {
            let flame_lvl = if car.perfect { 4 } else { car.boost_lvl };
            if tr.boost_prev == 0 {
                let n = 10 + 8 * car.boost_lvl as usize + if car.perfect { 10 } else { 0 };
                for _ in 0..n {
                    let r = &mut self.rng;
                    self.bits.push(Bit {
                        kind: Kind::Spark(flame_lvl),
                        p: car.p - fwd_v * 8.0,
                        v: -fwd_v * r.range(60.0, 150.0) + right_v * r.range(-70.0, 70.0),
                        dir: fwd_v,
                        z: car.z + 1.0,
                        vz: r.range(8.0, 24.0),
                        age: 0.0,
                        life: r.range(0.3, 0.55),
                        len: 0.0,
                    });
                }
                for k in 0..6 {
                    let side = if k % 2 == 0 { 1.0 } else { -1.0 };
                    let r = &mut self.rng;
                    self.puffs.push(Puff {
                        p: car.p - fwd_v * 6.0 + right_v * (side * 3.0),
                        v: right_v * (side * r.range(30.0, 55.0)) - fwd_v * r.range(10.0, 40.0),
                        z: 1.0,
                        vz: r.range(4.0, 12.0),
                        age: 0.0,
                        life: r.range(0.3, 0.5),
                        r0: 1.5,
                        r1: r.range(3.0, 5.0),
                        a: 0.6,
                        tone: if grass { 1 } else { 0 },
                    });
                }
            }
            let boost_size = if car.perfect { 1.25 } else { 1.0 };
            tr.flame += (140.0 + 40.0 * car.boost_lvl as f32) * dt;
            while tr.flame >= 1.0 && self.bits.len() < 900 {
                tr.flame -= 1.0;
                let r = &mut self.rng;
                self.bits.push(Bit {
                    kind: Kind::Flame(flame_lvl),
                    p: car.p - fwd_v * (8.0 + r.range(0.0, 4.0)) + right_v * r.range(-2.5, 2.5),
                    v: -fwd_v * r.range(70.0, 130.0) + right_v * r.range(-6.0, 6.0) + tr.vel * 0.3,
                    dir: fwd_v,
                    z: car.z + 2.0,
                    vz: r.next(),
                    age: 0.0,
                    life: r.range(0.2, 0.4),
                    len: r.range(0.5, 1.6) * boost_size,
                });
            }
        } else {
            tr.flame = 0.0;
        }
        if car.perfect && !tr.perfect_prev {
            for n in 0..26 {
                let ang = n as f32 / 26.0 * std::f32::consts::TAU;
                let d = right_v * ang.cos() + fwd_v * ang.sin();
                let r = &mut self.rng;
                self.bits.push(Bit {
                    kind: Kind::Spark(4),
                    p: car.p,
                    v: d * r.range(60.0, 130.0) + tr.vel * 0.4,
                    dir: fwd_v,
                    z: car.z + 2.0,
                    vz: r.range(10.0, 30.0),
                    age: 0.0,
                    life: r.range(0.35, 0.6),
                    len: 0.0,
                });
            }
        }
        tr.perfect_prev = car.perfect;

        if car.drift_dir != 0 && car.tier > tr.tier_prev {
            for n in 0..12 {
                let ang = n as f32 / 12.0 * std::f32::consts::TAU + self.rng.range(-0.2, 0.2);
                let d = right_v * ang.cos() + fwd_v * ang.sin();
                let r = &mut self.rng;
                self.bits.push(Bit {
                    kind: Kind::Spark(car.tier),
                    p: car.p - fwd_v * 3.0,
                    v: d * r.range(40.0, 85.0) + tr.vel * 0.5,
                    dir: fwd_v,
                    z: car.z + 2.0,
                    vz: r.range(10.0, 26.0),
                    age: 0.0,
                    life: r.range(0.25, 0.45),
                    len: 0.0,
                });
            }
        }
        if car.drift_dir != 0 && car.charge + 30 < tr.charge_prev {
            for n in 0..14 {
                let r = &mut self.rng;
                let ang = n as f32 / 14.0 * std::f32::consts::TAU;
                let d = right_v * ang.cos() + fwd_v * ang.sin();
                self.bits.push(Bit {
                    kind: Kind::Spark(5),
                    p: car.p,
                    v: d * r.range(25.0, 70.0) + tr.vel * 0.5,
                    dir: fwd_v,
                    z: car.z + 2.0,
                    vz: r.range(6.0, 20.0),
                    age: 0.0,
                    life: r.range(0.2, 0.4),
                    len: 0.0,
                });
            }
            for _ in 0..5 {
                let r = &mut self.rng;
                self.puffs.push(Puff {
                    p: car.p - fwd_v * 6.0 + Vector2::new(r.range(-3.0, 3.0), r.range(-3.0, 3.0)),
                    v: tr.vel * 0.3 + Vector2::new(r.range(-12.0, 12.0), r.range(-12.0, 12.0)),
                    z: car.z + 1.0,
                    vz: r.range(10.0, 20.0),
                    age: 0.0,
                    life: r.range(0.4, 0.7),
                    r0: 1.5,
                    r1: r.range(4.0, 6.0),
                    a: 0.6,
                    tone: 2,
                });
            }
        }
        tr.tier_prev = if car.drift_dir != 0 { car.tier } else { 0 };
        tr.charge_prev = car.charge;

        if car.heat > 50 && grounded {
            tr.soot += (car.heat as f32 - 50.0) / 50.0 * 12.0 * dt;
            while tr.soot >= 1.0 && self.puffs.len() < 400 {
                tr.soot -= 1.0;
                let r = &mut self.rng;
                self.puffs.push(Puff {
                    p: car.p - fwd_v * 9.0 + right_v * r.range(-1.5, 1.5),
                    v: tr.vel * 0.2 - fwd_v * r.range(10.0, 25.0),
                    z: car.z + 2.0,
                    vz: r.range(8.0, 16.0),
                    age: 0.0,
                    life: r.range(0.4, 0.8),
                    r0: 1.0,
                    r1: r.range(2.5, 4.0),
                    a: 0.5,
                    tone: 2,
                });
            }
        } else {
            tr.soot = 0.0;
        }
        tr.boost_prev = car.boost;

        for i in 0..4 {
            let k = if grounded {
                if i < 2 { front } else { rear }
            } else {
                0.0
            };
            let b = TIRE_POSITIONS[i];
            let w = Vector2::new(car.p.x + b.x * c - b.y * s, car.p.y + b.x * s + b.y * c);

            if k > 0.05 && !grass {
                if let Some(l) = tr.last[i] {
                    if (w - l).length() < 14.0 {
                        let amt = k * 0.50 * (0.8 + 0.4 * self.rng.next());
                        self.pending.push((
                            l - self.origin,
                            w - self.origin,
                            (amt * 255.0).min(255.0) as u8,
                        ));
                    }
                }
                tr.last[i] = Some(w);
            } else {
                tr.last[i] = None;
            }

            let min_k = if i < 2 { 0.4 } else { 0.3 };
            if grass && k > min_k && slip > 22.0 && self.puffs.len() < 400 {
                tr.smoke[i] += 12.0 * k * dt;
                while tr.smoke[i] >= 1.0 {
                    tr.smoke[i] -= 1.0;
                    let r = &mut self.rng;
                    self.puffs.push(Puff {
                        p: w + Vector2::new(r.range(-1.0, 1.0), r.range(-1.0, 1.0)),
                        v: tr.vel * 0.3 + Vector2::new(r.range(-8.0, 8.0), r.range(-8.0, 8.0)),
                        z: car.z + 1.0,
                        vz: r.range(10.0, 22.0),
                        age: 0.0,
                        life: r.range(0.5, 0.9) * (0.8 + 0.4 * k),
                        r0: 1.5,
                        r1: r.range(4.0, 6.0) * (0.7 + 0.5 * k),
                        a: (0.35 + 0.35 * k).min(0.75),
                        tone: 1,
                    });
                }
            } else {
                tr.smoke[i] = 0.0;
            }

            if i >= 2 {
                let n = i - 2;
                if drifting && !grass && self.puffs.len() < 400 && slide > 0.3 {
                    tr.dsmoke[n] += (5.0 + 14.0 * frac + 6.0 * car.tier as f32) * slide * dt;
                    while tr.dsmoke[n] >= 1.0 {
                        tr.dsmoke[n] -= 1.0;
                        let r = &mut self.rng;
                        self.puffs.push(Puff {
                            p: w + Vector2::new(r.range(-1.0, 1.0), r.range(-1.0, 1.0)),
                            v: tr.vel * 0.15 + Vector2::new(r.range(-6.0, 6.0), r.range(-6.0, 6.0)),
                            z: car.z + 1.0,
                            vz: r.range(8.0, 16.0),
                            age: 0.0,
                            life: r.range(0.3, 0.55),
                            r0: 1.0,
                            r1: r.range(2.5, 4.0) * (0.8 + 0.6 * frac),
                            a: 0.4 + 0.2 * frac,
                            tone: 0,
                        });
                    }
                } else {
                    tr.dsmoke[n] = 0.0;
                }
                if drifting && self.bits.len() < 700 {
                    let warn = if car.over > 20 { 60.0 } else { 0.0 };
                    tr.spark[n] += (8.0 + 25.0 * frac + 25.0 * car.tier as f32 + warn) * dt;
                    while tr.spark[n] >= 1.0 {
                        tr.spark[n] -= 1.0;
                        let r = &mut self.rng;
                        self.bits.push(Bit {
                            kind: Kind::Spark(if car.over > 20 && car.tier == 3 {
                                0
                            } else {
                                car.tier
                            }),
                            p: w,
                            v: -fwd_v * r.range(20.0, 55.0)
                                - right_v * (car.drift_dir as f32 * r.range(10.0, 35.0))
                                + tr.vel * 0.5,
                            dir: fwd_v,
                            z: car.z + 1.0,
                            vz: r.range(8.0, 24.0),
                            age: 0.0,
                            life: r.range(0.18, 0.35),
                            len: 0.0,
                        });
                    }
                } else {
                    tr.spark[n] = 0.0;
                }
            }
        }
    }
    fn landing_burst(&mut self, p: Vector2, strength: f32, grass: bool) {
        for n in 0..8 {
            let ang = n as f32 / 8.0 * std::f32::consts::TAU + self.rng.range(-0.3, 0.3);
            let dir = Vector2::new(ang.cos(), ang.sin());
            let r = &mut self.rng;
            self.puffs.push(Puff {
                p: p + dir * 6.0,
                v: dir * r.range(20.0, 34.0) * strength,
                z: 1.0,
                vz: r.range(6.0, 14.0),
                age: 0.0,
                life: r.range(0.4, 0.7),
                r0: 1.5,
                r1: r.range(3.5, 5.5) * strength.max(0.6),
                a: 0.55,
                tone: if grass { 1 } else { 0 },
            });
        }
    }
    fn splash(&mut self, p: Vector2, s: f32) {
        let n = (7.0 + 14.0 * s) as usize;
        for i in 0..n {
            let ang = i as f32 / n as f32 * std::f32::consts::TAU + self.rng.range(-0.25, 0.25);
            let dir = Vector2::new(ang.cos(), ang.sin());
            let r = &mut self.rng;
            self.droplets.push(Droplet {
                p: p + dir * 3.0,
                v: dir * r.range(14.0, 30.0) * (0.5 + s),
                z: 0.0,
                vz: r.range(70.0, 105.0) * (0.55 + 0.6 * s),
                big: r.next() < 0.35 + 0.3 * s,
            });
        }
        if s > 0.35 {
            for _ in 0..(2.0 + 8.0 * s * s) as usize {
                let r = &mut self.rng;
                self.droplets.push(Droplet {
                    p: p + Vector2::new(r.range(-1.5, 1.5), r.range(-1.5, 1.5)),
                    v: Vector2::new(r.range(-6.0, 6.0), r.range(-6.0, 6.0)),
                    z: 0.0,
                    vz: r.range(100.0, 135.0) * (0.6 + 0.5 * s),
                    big: r.next() < 0.6,
                });
            }
        }
        if self.ripples.len() < 60 {
            let r1 = 7.0 + 14.0 * s;
            self.ripples.push(Ripple {
                p,
                age: 0.0,
                life: 0.55,
                r1,
            });
            self.ripples.push(Ripple {
                p,
                age: -0.12,
                life: 0.6,
                r1: r1 * 0.7,
            });
        }
    }

    // ------------------------------------------------------------------ draw

    pub fn flush(&mut self, ctx: &mut Context) {
        if self.pending.is_empty() {
            return;
        }
        let mut tm = ctx.rl.begin_texture_mode(&ctx.thread, &mut self.marks);
        let mut bm = tm.begin_blend_mode(BlendMode::BLEND_ADDITIVE);
        for (a, b, al) in self.pending.drain(..) {
            bm.draw_line_ex(a, b, 1.3, Color::new(255, 255, 255, al));
        }
    }

    pub fn draw_marks(
        &mut self,
        tm: &mut RaylibTextureMode<'_, '_, RaylibHandle>,
        camera: &Camera,
    ) {
        let (o, inv) = camera.inverse();
        self.shader.set_shader_value(self.loc_origin, o);
        self.shader.set_shader_value(self.loc_inv, inv);
        self.shader.set_shader_value(self.loc_morigin, self.origin);
        self.shader.set_shader_value(self.loc_msize, self.size);
        self.shader
            .set_shader_value_texture(self.loc_marks, self.marks.texture());

        unsafe { rlDisableBackfaceCulling() }
        {
            let mut sm = tm.begin_shader_mode(&mut self.shader);
            let (a, b, c, d) = (
                Vector2::new(0., 0.),
                Vector2::new(0., 180.),
                Vector2::new(320., 180.),
                Vector2::new(320., 0.),
            );
            sm.draw_triangle(a, b, c, Color::WHITE);
            sm.draw_triangle(a, c, d, Color::WHITE);
        }
        unsafe { rlEnableBackfaceCulling() }
    }
    pub fn draw_puffs(&self, tm: &mut RaylibTextureMode<'_, '_, RaylibHandle>, camera: &Camera) {
        for p in &self.puffs {
            let t = p.age / p.life;
            let r = (p.r0 + (p.r1 - p.r0) * (1.0 - (1.0 - t).powi(2))).round();
            let band = ((p.a * (1.0 - t).powf(1.3)) * 4.0).ceil() / 4.0;
            if band <= 0.0 {
                continue;
            }
            let al = (band * 255.0) as u8;

            let (s, _) = camera.apply(p.p, 0.0);
            let (x, y) = (s.x.round() as i32, (s.y - p.z).round() as i32);
            let ((br, bg, bb), (sr, sg, sb), (hr, hg, hb)) = match p.tone {
                1 => ((158, 146, 108), (122, 110, 78), (190, 178, 140)),
                2 => ((70, 68, 74), (40, 38, 44), (104, 100, 108)),
                _ => ((232, 232, 236), (176, 178, 190), (250, 250, 252)),
            };
            tm.draw_circle(x + 1, y + 1, r, Color::new(sr, sg, sb, al));
            tm.draw_circle(x, y, r, Color::new(br, bg, bb, al));
            if r > 3.0 {
                tm.draw_circle(x - 1, y - 1, (r * 0.5).round(), Color::new(hr, hg, hb, al));
            }
        }
    }
    pub fn draw_bits(&self, tm: &mut RaylibTextureMode<'_, '_, RaylibHandle>, camera: &Camera) {
        for b in &self.bits {
            let t = b.age / b.life;
            let proj = |p: Vector2| {
                let (s, _) = camera.apply(p, 0.0);
                Vector2::new(s.x.round(), (s.y - b.z).round())
            };
            match b.kind {
                Kind::Trail => continue,
                Kind::Spark(tier) => {
                    let ((r, g, bl), (hr, hg, hb)) = Self::tier_cols(tier);
                    let al = (((1.0 - t) * 4.0).ceil() / 4.0 * 255.0) as u8;
                    let a = proj(b.p);
                    let e = proj(b.p - b.v * 0.06);
                    tm.draw_line_ex(
                        a,
                        e,
                        if t < 0.5 { 2.0 } else { 1.0 },
                        Color::new(r, g, bl, al),
                    );
                    tm.draw_line_ex(a, e, 1.0, Color::new(hr, hg, hb, al));
                }
                Kind::Flame(_) => continue,
            }
        }
    }
    pub fn draw_trails(&self, tm: &mut RaylibTextureMode<'_, '_, RaylibHandle>, camera: &Camera) {
        let pr = |p: Vector2| {
            let (s, _) = camera.apply(p, 0.0);
            Vector2::new(s.x.round(), s.y.round())
        };
        for b in &self.bits {
            if !matches!(b.kind, Kind::Trail) {
                continue;
            }
            let t = b.age / b.life;
            let seed = b.vz;

            let h = (seed * 97.0).fract();
            if t > 0.6 && h < (t - 0.6) * 2.5 {
                continue;
            }

            let ang = ((seed * 13.7).fract() - 0.5) * 1.0;
            let (sn, cs) = ang.sin_cos();
            let d = Vector2::new(b.dir.x * cs - b.dir.y * sn, b.dir.x * sn + b.dir.y * cs);
            let n = Vector2::new(-d.y, d.x);
            let l = b.len * (1.0 - t * 0.4);
            let col = if t > 0.5 {
                Color::new(215, 220, 232, 255)
            } else {
                Color::WHITE
            };

            match (seed * 4.0) as u32 {
                0 => tm.draw_line_ex(pr(b.p), pr(b.p + d * l), 1.0, col),
                1 => {
                    tm.draw_line_ex(pr(b.p), pr(b.p + d * (l * 0.55)), 2.0, col);
                    let o = b.p + n * 1.5 + d * (l * 0.6);
                    tm.draw_line_ex(pr(o), pr(o + d * (l * 0.5)), 1.0, col);
                }
                2 => tm.draw_line_ex(pr(b.p), pr(b.p + d * (l * 0.4)), 2.0, col),
                _ => {
                    tm.draw_line_ex(pr(b.p), pr(b.p + d * (l * 0.35)), 2.0, col);
                    tm.draw_line_ex(pr(b.p + d * (l * 0.35)), pr(b.p + d * l), 1.0, col);
                }
            }
        }
    }
    pub fn draw_flames(&self, tm: &mut RaylibTextureMode<'_, '_, RaylibHandle>, camera: &Camera) {
        for layer in 0..3 {
            for b in &self.bits {
                let Kind::Flame(lvl) = b.kind else {
                    continue;
                };
                let t = b.age / b.life;

                if t > 0.55 && (b.vz * 97.0).fract() < (t - 0.55) * 2.2 {
                    continue;
                }

                let pal = Self::flame_pal(lvl);
                let (scale, col) = match layer {
                    0 => (1.0, if t < 0.5 { pal[0].0 } else { pal[0].1 }),
                    1 if t < 0.7 => (0.7, if t < 0.3 { pal[1].0 } else { pal[1].1 }),
                    2 if t < 0.4 => (0.4, if t < 0.12 { pal[2].0 } else { pal[2].1 }),
                    _ => continue,
                };

                let r = (b.len * 2.6 * (1.0 - t).powf(0.7) * scale).round();
                if r < 1.0 {
                    continue;
                }

                let (s, _) = camera.apply(b.p, 0.0);
                let (x, y) = (s.x.round(), (s.y - b.z).round());
                let c = Color::new(col.0, col.1, col.2, 255);

                let sp = b.v.length().max(1.0);
                let vn = b.v * (1.0 / sp);
                let tip = Vector2::new((x + vn.x * r * 1.1).round(), (y + vn.y * r * 1.1).round());
                tm.draw_circle(x as i32, y as i32, r, c);
                tm.draw_circle(tip.x as i32, tip.y as i32, (r * 0.6).round().max(1.0), c);
            }
        }
    }
    pub fn draw_ripples(
        &self,
        tm: &mut RaylibTextureMode<'_, '_, RaylibHandle>,
        camera: &Camera,
        water: &dyn Fn(Vector2) -> bool,
    ) {
        for r in &self.ripples {
            if r.age < 0.0 {
                continue;
            }
            let t = r.age / r.life;
            let rad = 1.5 + (r.r1 - 1.5) * (1.0 - (1.0 - t).powi(2));
            let n = ((std::f32::consts::TAU * rad) as usize).max(8);
            let col = if t < 0.5 { W_FOAM } else { W_LIGHT };
            for i in 0..n {
                if t > 0.45 && (i * 7 + (t * 6.0) as usize * 13) % 5 < ((t - 0.45) * 8.0) as usize {
                    continue;
                }
                let ang = i as f32 / n as f32 * std::f32::consts::TAU;
                let w = r.p + Vector2::new(ang.cos(), ang.sin()) * rad;
                if !water(w) {
                    continue;
                }
                let (s, _) = camera.apply(w, 0.0);
                let (x, y) = (s.x.round() as i32, s.y.round() as i32);
                tm.draw_pixel(x, y + 1, W_DARK);
                tm.draw_pixel(x, y, col);
            }
        }
    }
    pub fn draw_droplets(
        &self,
        tm: &mut RaylibTextureMode<'_, '_, RaylibHandle>,
        camera: &Camera,
        water: &dyn Fn(Vector2) -> bool,
    ) {
        for d in &self.droplets {
            let proj = |p: Vector2, z: f32| {
                let (s, _) = camera.apply(p, 0.0);
                Vector2::new(s.x.round(), (s.y - z).round())
            };
            let head = proj(d.p, d.z);
            let rising = d.vz > 0.0;

            if d.z > 2.0 && water(d.p) {
                let g = proj(d.p, 0.0);
                tm.draw_pixel(g.x as i32, g.y as i32, W_DARK);
            }
            if rising {
                let tail = proj(d.p - d.v * 0.035, d.z - d.vz * 0.035);
                tm.draw_line_ex(head, tail, 1.0, W_LIGHT);
            }
            let (x, y) = (head.x as i32, head.y as i32);
            if d.big {
                tm.draw_rectangle(x, y, 2, 2, if rising { W_FOAM } else { W_LIGHT });
                tm.draw_pixel(x + 1, y + 1, W_LIGHT);
            } else {
                tm.draw_pixel(x, y, if rising { W_FOAM } else { W_LIGHT });
            }
        }
    }

    fn flame_pal(lvl: u8) -> [((u8, u8, u8), (u8, u8, u8)); 3] {
        match lvl {
            1 => [
                ((40, 120, 255), (25, 60, 210)),
                ((90, 190, 255), (60, 150, 255)),
                ((225, 245, 255), (160, 215, 255)),
            ],
            3 => [
                ((195, 85, 255), (115, 45, 205)),
                ((235, 140, 255), (200, 100, 255)),
                ((255, 235, 255), (240, 190, 255)),
            ],
            4 => [
                ((255, 190, 40), (235, 105, 20)),
                ((255, 235, 110), (255, 200, 60)),
                ((255, 255, 235), (255, 245, 190)),
            ],
            _ => [
                ((255, 120, 25), (200, 45, 30)),
                ((255, 190, 50), (255, 150, 35)),
                ((255, 250, 215), (255, 224, 90)),
            ],
        }
    }
    fn tier_cols(t: u8) -> ((u8, u8, u8), (u8, u8, u8)) {
        match t {
            0 => ((255, 240, 210), (255, 255, 255)),
            1 => ((60, 160, 255), (140, 215, 255)),
            2 => ((255, 150, 40), (255, 220, 120)),
            3 => ((190, 90, 255), (230, 170, 255)),
            4 => ((255, 200, 50), (255, 250, 200)),
            _ => ((230, 60, 50), (255, 150, 120)),
        }
    }
}
