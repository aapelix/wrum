use std::collections::HashMap;

use raylib::{
    RaylibHandle,
    color::Color,
    drawing::{RaylibDraw, RaylibShaderModeExt, RaylibTextureMode},
    ffi::{Vector3, rlDisableBackfaceCulling, rlEnableBackfaceCulling},
    math::{Rectangle, Vector2},
    shaders::{RaylibShader, Shader},
    texture::RaylibTexture2D,
};
use wrum_core::map::{PropKind::Fence, load_map};

use crate::{
    GLSL_VERSION, assets::Assets, camera::Camera, car::SHADOW_OFFSET, ctx::Context, stack::Stack,
};

struct PropKind {
    stack: Stack,
}

pub struct Prop {
    kind: usize,
    pub pos: Vector2,
    pub a: f32,
    pub z: f32,
}

pub struct Map {
    layers: Stack,
    kinds: Vec<PropKind>,
    pub props: Vec<Prop>,
    water: Vec<Rectangle>,

    water_shader: Shader,
    loc_time: i32,
    loc_origin: i32,
    loc_inv: i32,
    loc_clip: i32,
    loc_alpha: i32,
    pub size: Vector2,
}

impl Map {
    pub fn load(ctx: &mut Context, path: &str, data_path: &str, layers: i8) -> Map {
        let map_json = load_map(data_path).expect("failed to load map");

        let mut props = Vec::new();
        let mut kinds: Vec<PropKind> = Vec::new();
        let mut kind_indices = HashMap::new();
        let mut water = Vec::new();

        for w in map_json.water {
            let (hw, hh) = (w.2 / 2., w.3 / 2.);
            water.push(Rectangle {
                x: w.0 - hw,
                y: w.1 - hh,
                width: w.2,
                height: w.3,
            });
        }
        for p in map_json.props {
            let kind = *kind_indices.entry(p.kind).or_insert_with(|| {
                let index = kinds.len();
                kinds.push(PropKind {
                    stack: match p.kind {
                        wrum_core::map::PropKind::Fence => Stack::new(ctx, "assets/fence.png", 17.),
                        wrum_core::map::PropKind::FenceEnd => {
                            Stack::new(ctx, "assets/fence_end.png", 4.)
                        }
                        wrum_core::map::PropKind::Tires => Stack::new(ctx, "assets/tires.png", 24.),
                        wrum_core::map::PropKind::Tree => Stack::new(ctx, "assets/tree.png", 42.),
                        wrum_core::map::PropKind::Rock => Stack::new(ctx, "assets/rock.png", 16.),
                        wrum_core::map::PropKind::Fern => Stack::new(ctx, "assets/fern.png", 17.),
                    },
                });
                index
            });

            let pos = match p.kind {
                Fence => {
                    let rot = p.rotation.to_radians();
                    let offset = Vector2::new(6.5, 0.0);
                    let (s, c) = libm::sincosf(rot);
                    let offset =
                        Vector2::new(offset.x * c - offset.y * s, offset.x * s + offset.y * c);
                    Vector2 { x: p.x, y: p.y } + offset
                }
                _ => Vector2 { x: p.x, y: p.y },
            };

            props.push(Prop {
                kind,
                pos,
                a: p.rotation,
                z: 0.,
            });
        }

        let shader_dir = format!("assets/shaders/glsl{}", GLSL_VERSION);
        let water_shader = ctx.rl.load_shader(
            &ctx.thread,
            Some(&format!("{shader_dir}/water.vs")),
            Some(&format!("{shader_dir}/water.fs")),
        );

        let loc_time = water_shader.get_shader_location("uTime");
        let loc_origin = water_shader.get_shader_location("uOrigin");
        let loc_inv = water_shader.get_shader_location("uInv");
        let loc_clip = water_shader.get_shader_location("uClip");
        let loc_alpha = water_shader.get_shader_location("uAlpha");

        let (w, h) = {
            let tex = ctx.assets.load(ctx.rl, ctx.thread, path);
            let tex = ctx.assets.get(&tex);
            (tex.width() as f32, tex.height() as f32)
        };
        let layers = Stack::new(ctx, path, w / layers as f32);

        Self {
            layers,
            kinds,
            props,
            water,
            water_shader,
            loc_time,
            loc_origin,
            loc_inv,
            loc_alpha,
            loc_clip,
            size: Vector2 { x: w, y: h },
        }
    }

    pub fn visible(prop: &Prop, bounds: Rectangle) -> bool {
        prop.pos.x >= bounds.x
            && prop.pos.x <= bounds.x + bounds.width
            && prop.pos.y >= bounds.y
            && prop.pos.y <= bounds.y + bounds.height
    }

    pub fn draw_layers(
        &mut self,
        assets: &Assets,
        tm: &mut RaylibTextureMode<'_, '_, RaylibHandle>,
        camera: &Camera,
    ) {
        self.draw_water(tm, camera);
        self.layers.draw(
            assets,
            tm,
            Vector2 {
                x: self.size.x / 2.,
                y: self.size.y / 2.,
            },
            0.,
            camera,
            0.,
        );
    }
    pub fn draw_water(
        &mut self,
        tm: &mut RaylibTextureMode<'_, '_, RaylibHandle>,
        camera: &Camera,
    ) {
        if self.water.is_empty() {
            return;
        }
        let bounds = camera.visible_bounds(0.0);

        let visible: Vec<Rectangle> = self
            .water
            .iter()
            .filter(|w| {
                w.x < bounds.x + bounds.width
                    && w.x + w.width > bounds.x
                    && w.y < bounds.y + bounds.height
                    && w.y + w.height > bounds.y
            })
            .cloned()
            .collect();

        if visible.is_empty() {
            return;
        }

        self.upload_view(tm, camera);
        self.water_shader
            .set_shader_value(self.loc_clip, Vector3::new(0., 0., 0.));
        self.water_shader.set_shader_value(self.loc_alpha, 1.0f32);
        self.draw_rects(tm, camera, &visible);
    }
    pub fn draw_water_overlay(
        &mut self,
        tm: &mut RaylibTextureMode<'_, '_, RaylibHandle>,
        camera: &Camera,
        world: Vector2,
        depth: f32,
    ) {
        let r = 16.0 + depth * 0.5;
        let m = r + depth;
        let near: Vec<Rectangle> = self
            .water
            .iter()
            .filter(|w| {
                world.x >= w.x - m
                    && world.x <= w.x + w.width + m
                    && world.y >= w.y - m
                    && world.y <= w.y + w.height + m
            })
            .cloned()
            .collect();
        if near.is_empty() {
            return;
        }

        self.upload_view(tm, camera);
        let (c, _) = camera.apply(world, 0.0);
        let alpha = (0.55 + 0.4 * depth / 24.0).clamp(0.55, 0.95);
        self.water_shader
            .set_shader_value(self.loc_clip, Vector3::new(c.x, c.y + depth * 0.5, r));
        self.water_shader.set_shader_value(self.loc_alpha, alpha);
        self.draw_rects(tm, camera, &near);

        self.water_shader
            .set_shader_value(self.loc_clip, Vector3::new(0., 0., 0.));
        self.water_shader.set_shader_value(self.loc_alpha, 1.0f32);
    }
    pub fn draw_shadows(
        &mut self,
        tm: &mut RaylibTextureMode<'_, '_, RaylibHandle>,
        camera: &Camera,
    ) {
        let bounds = camera.visible_bounds(32.0);

        for prop in &self.props {
            if !Self::visible(prop, bounds) {
                continue;
            }

            let kind = &self.kinds[prop.kind];
            kind.stack
                .draw_shadow(tm, prop.pos, prop.a, camera, SHADOW_OFFSET, 255);
        }
    }
    pub fn draw_prop(
        &mut self,
        assets: &Assets,
        tm: &mut RaylibTextureMode<'_, '_, RaylibHandle>,
        i: usize,
        camera: &Camera,
    ) {
        let prop = &self.props[i];
        let kind = &self.kinds[prop.kind];
        kind.stack.draw(
            assets,
            tm,
            Vector2 {
                x: prop.pos.x,
                y: prop.pos.y,
            },
            prop.a,
            camera,
            prop.z,
        );
    }

    fn upload_view(&mut self, tm: &RaylibTextureMode<'_, '_, RaylibHandle>, camera: &Camera) {
        let (o, _) = camera.apply(Vector2::zero(), 0.);
        let ex = camera.apply(Vector2::new(1., 0.), 0.).0 - o;
        let ey = camera.apply(Vector2::new(0., 1.), 0.).0 - o;
        let det = ex.x * ey.y - ex.y * ey.x;
        let inv = [ey.y / det, -ey.x / det, -ex.y / det, ex.x / det];
        self.water_shader
            .set_shader_value(self.loc_time, tm.get_time() as f32);
        self.water_shader.set_shader_value(self.loc_origin, o);
        self.water_shader.set_shader_value(self.loc_inv, inv);
    }
    fn draw_rects(
        &mut self,
        tm: &mut RaylibTextureMode<'_, '_, RaylibHandle>,
        camera: &Camera,
        rects: &[Rectangle],
    ) {
        unsafe { rlDisableBackfaceCulling() }
        {
            let mut sm = tm.begin_shader_mode(&mut self.water_shader);
            for rect in rects {
                let (a, _) = camera.apply(Vector2::new(rect.x, rect.y), 0.);
                let (b, _) = camera.apply(Vector2::new(rect.x, rect.y + rect.height), 0.);
                let (c, _) =
                    camera.apply(Vector2::new(rect.x + rect.width, rect.y + rect.height), 0.);
                let (d, _) = camera.apply(Vector2::new(rect.x + rect.width, rect.y), 0.);
                sm.draw_triangle(a, b, c, Color::WHITE);
                sm.draw_triangle(a, c, d, Color::WHITE);
            }
        }
        unsafe { rlEnableBackfaceCulling() }
    }
}
