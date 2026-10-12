use raylib::{
    RaylibHandle,
    color::Color,
    drawing::{RaylibDraw, RaylibShaderModeExt, RaylibTextureMode},
    ffi::{rlDisableBackfaceCulling, rlEnableBackfaceCulling},
    math::{Rectangle, Vector2},
    shaders::{RaylibShader, Shader},
    texture::{RaylibRenderTexture2D, RenderTexture2D, Texture2D},
};
use wrum_core::map::{LayerJson, ObjectKindJson, load_map};

use crate::{GLSL_VERSION, camera::Camera, car::SHADOW_OFFSET, ctx::Context, stack::Stack};

pub const TILE_SIZE: f32 = 16.;
pub const CENTER: Vector2 = Vector2 {
    x: TILE_SIZE / 2.,
    y: TILE_SIZE / 2.,
};
pub const GROUND_SHADOW_OFFSET: Vector2 = Vector2 { x: 0., y: 7. };

pub struct Tile {
    pos: Vector2,
    src: Rectangle,
}
pub struct Layer {
    tiles: Vec<Tile>,
}
pub struct Prop {
    stack: Stack,
    pub pos: Vector2,
    pub a: f32,
}

pub struct Map {
    layers: Vec<Layer>,
    tileset: Texture2D,
    pub props: Vec<Prop>,
    water: Vec<Rectangle>,

    water_shader: Shader,
    loc_time: i32,
    loc_origin: i32,
    loc_inv: i32,
}

impl Map {
    pub fn load(ctx: &mut Context, path: &str) -> Map {
        let map_json = load_map(path).expect("failed to load map");

        let tileset_json = map_json.tilesets.first().expect("map has no tilesets");
        let tiles_per_row = tileset_json.imagewidth / tileset_json.tilewidth;

        let mut layers = Vec::new();
        let mut props = Vec::new();
        let mut water = Vec::new();

        for layer in map_json.layers {
            match layer {
                LayerJson::TileLayer(layer) => {
                    let mut tiles = Vec::new();

                    for (i, index) in layer.data.into_iter().enumerate() {
                        if index == 0 {
                            continue;
                        }

                        let x = (i as u32 % layer.width) as f32 * TILE_SIZE;
                        let y = (i as u32 / layer.width) as f32 * TILE_SIZE;

                        let index = index - 1;

                        let col = index % tiles_per_row;
                        let row = index / tiles_per_row;

                        let src = Rectangle {
                            x: col as f32 * TILE_SIZE,
                            y: row as f32 * TILE_SIZE,
                            width: TILE_SIZE,
                            height: TILE_SIZE,
                        };

                        tiles.push(Tile {
                            pos: Vector2::new(x, y),
                            src,
                        });
                    }

                    layers.push(Layer { tiles });
                }
                LayerJson::ObjectLayer(layer) => {
                    for object in layer.objects {
                        match object.kind {
                            ObjectKindJson::Fence { .. } => {
                                let stack = Stack::new(ctx, "assets/fence.png", 20.);

                                props.push(Prop {
                                    stack,
                                    pos: Vector2::new(object.x, object.y),
                                    a: 0.0,
                                });
                            }
                            ObjectKindJson::Water => {
                                water.push(Rectangle {
                                    x: object.x,
                                    y: object.y,
                                    width: object.width,
                                    height: object.height,
                                });
                            }
                            _ => {}
                        }
                    }
                }
            }
        }

        let tileset = ctx
            .rl
            .load_texture(&ctx.thread, "assets/tiles.png")
            .expect("failed to load tileset");

        let shader_dir = format!("assets/shaders/glsl{}", GLSL_VERSION);
        let water_shader = ctx.rl.load_shader(
            &ctx.thread,
            Some(&format!("{shader_dir}/water.vs")),
            Some(&format!("{shader_dir}/water.fs")),
        );

        let loc_time = water_shader.get_shader_location("uTime");
        let loc_origin = water_shader.get_shader_location("uOrigin");
        let loc_inv = water_shader.get_shader_location("uInv");

        Self {
            layers,
            tileset,
            props,
            water,
            water_shader,
            loc_time,
            loc_origin,
            loc_inv,
        }
    }

    pub fn draw_layers(
        &mut self,
        tm: &mut RaylibTextureMode<'_, '_, RaylibHandle>,
        camera: &Camera,
    ) {
        self.draw_water(tm, camera);

        for layer in &self.layers {
            for tile in &layer.tiles {
                let (pos, a) = camera.apply(tile.pos, 0.);
                tm.draw_texture_pro(
                    &self.tileset,
                    tile.src,
                    Rectangle::new(pos.x, pos.y, TILE_SIZE, TILE_SIZE),
                    CENTER,
                    a,
                    Color::WHITE,
                );
            }
        }
    }
    pub fn draw_water(
        &mut self,
        tm: &mut RaylibTextureMode<'_, '_, RaylibHandle>,
        camera: &Camera,
    ) {
        if self.water.len() == 0 {
            return;
        }

        let (o, _) = camera.apply(Vector2::zero(), 0.);
        let ex = camera.apply(Vector2::new(1., 0.), 0.).0 - o;
        let ey = camera.apply(Vector2::new(0., 1.), 0.).0 - o;
        let det = ex.x * ey.y - ex.y * ey.x;
        let inv = [ey.y / det, -ey.x / det, -ex.y / det, ex.x / det];
        let t = tm.get_time() as f32;

        self.water_shader.set_shader_value(self.loc_time, t);
        self.water_shader.set_shader_value(self.loc_origin, o);
        self.water_shader.set_shader_value(self.loc_inv, inv);

        unsafe { rlDisableBackfaceCulling() }
        {
            let mut sm = tm.begin_shader_mode(&mut self.water_shader);
            for rect in &self.water {
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
    pub fn draw_shadows(
        &mut self,
        tm: &mut RaylibTextureMode<'_, '_, RaylibHandle>,
        camera: &Camera,
    ) {
        for prop in &self.props {
            prop.stack
                .draw_shadow(tm, prop.pos, prop.a, camera, SHADOW_OFFSET);
        }
    }
    pub fn draw_prop(
        &mut self,
        tm: &mut RaylibTextureMode<'_, '_, RaylibHandle>,
        i: usize,
        camera: &Camera,
    ) {
        let prop = &self.props[i];
        prop.stack.draw(tm, prop.pos, prop.a, camera, 0.);
    }
}
