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
    physics::{Status, Vec2},
};

use crate::{
    GLSL_VERSION,
    camera::Camera,
    car::{Car, TIRE_POSITIONS},
    ctx::Context,
};

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
    dust: bool,
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
}

pub struct Fx {
    pub marks: RenderTexture2D,
    origin: Vector2,
    pub size: Vector2,
    pending: Vec<(Vector2, Vector2, u8)>,
    puffs: Vec<Puff>,
    tracks: HashMap<PlayerId, Track>,
    rng: Rng,

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
        }
    }

    // ---------------------------------------------------------------- update

    pub fn update(&mut self, dt: f32, cars: &HashMap<PlayerId, Car>, level: &Level) {
        if dt <= 0.0 {
            return;
        }
        self.tracks.retain(|id, _| cars.contains_key(id));

        for (id, car) in cars {
            let grass = level.on_grass(Vec2::new(car.p.x, car.p.y));
            let mut tr = self.tracks.remove(id).unwrap_or_default();
            self.step_car(&mut tr, car, dt, grass);
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
    }

    fn step_car(&mut self, tr: &mut Track, car: &Car, dt: f32, grass: bool) {
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
        if up {
            tr.peak_z = tr.peak_z.max(car.z);
        }
        if tr.airborne && !up {
            if tr.peak_z > 6.0 {
                tr.land = (tr.peak_z / 25.0).clamp(0.3, 1.0);
                self.landing_burst(car.p, tr.land, grass);
            }
            tr.peak_z = 0.0;
        }
        tr.airborne = up;

        let grounded = !up && car.z > -0.01 && car.status != Status::Drowning;

        let slide = smooth(22.0, 65.0, slip);
        let hard = if fwd > 30.0 {
            smooth(200.0, 380.0, tr.decel)
        } else {
            0.0
        };
        let spun = if car.status == Status::Spun { 0.9 } else { 0.0 };
        tr.land = (tr.land - dt * 4.0).max(0.0);
        let boost = if fwd.abs() > 60.0 { tr.land * 0.8 } else { 0.0 };

        let rear = slide.max(hard).max(spun).max(boost);
        let front = (slide * smooth(40.0, 90.0, slip))
            .max(hard * 0.7)
            .max(spun * 0.8)
            * 0.6;

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
            if k > min_k && (!grass || slip > 22.0) && self.puffs.len() < 400 {
                tr.smoke[i] += 12.0 * k * dt;
                while tr.smoke[i] >= 1.0 {
                    tr.smoke[i] -= 1.0;
                    let r = &mut self.rng;
                    self.puffs.push(Puff {
                        p: w + Vector2::new(r.range(-1.0, 1.0), r.range(-1.0, 1.0)),
                        v: tr.vel * 0.3 + Vector2::new(r.range(-8.0, 8.0), r.range(-8.0, 8.0)),
                        z: 1.0,
                        vz: r.range(10.0, 22.0),
                        age: 0.0,
                        life: r.range(0.5, 0.9) * (0.8 + 0.4 * k),
                        r0: 1.5,
                        r1: r.range(4.0, 6.0) * (0.7 + 0.5 * k),
                        a: (0.35 + 0.35 * k).min(0.75),
                        dust: grass,
                    });
                }
            } else {
                tr.smoke[i] = 0.0;
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
                dust: grass,
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
            let ((br, bg, bb), (sr, sg, sb), (hr, hg, hb)) = if p.dust {
                ((158, 146, 108), (122, 110, 78), (190, 178, 140))
            } else {
                ((232, 232, 236), (176, 178, 190), (250, 250, 252))
            };
            tm.draw_circle(x + 1, y + 1, r, Color::new(sr, sg, sb, al));
            tm.draw_circle(x, y, r, Color::new(br, bg, bb, al));
            if r > 3.0 {
                tm.draw_circle(x - 1, y - 1, (r * 0.5).round(), Color::new(hr, hg, hb, al));
            }
        }
    }
}
