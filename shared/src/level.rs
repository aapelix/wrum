use crate::{
    coll::{Obb, Vec2},
    map::{LayerJson, ObjectKindJson, load_map},
    sim::CarState,
};

pub const WALL_RESTITUTION: f32 = 0.4;

#[derive(Clone)]
pub struct Level {
    walls: Vec<Obb>,
}

impl Level {
    pub fn load(path: &str) -> Self {
        let Ok(map) = load_map(path) else {
            return Self { walls: Vec::new() };
        };

        let mut walls = Vec::new();
        for layer in &map.layers {
            match layer {
                LayerJson::ObjectLayer(layer) => {
                    for obj in &layer.objects {
                        match obj.kind {
                            ObjectKindJson::Wall => {
                                walls.push(obb_from_tiled_rect(
                                    obj.x,
                                    obj.y,
                                    obj.width,
                                    obj.height,
                                    obj.rotation,
                                ));
                            }
                            _ => {}
                        }
                    }
                }
                _ => {}
            }
        }

        Self { walls }
    }
    pub fn resolve_car(&self, car: &mut CarState) {
        for wall in &self.walls {
            car.resolve_vs_obstacle(*wall, WALL_RESTITUTION);
        }
    }
}

fn obb_from_tiled_rect(x: f32, y: f32, w: f32, h: f32, a: f32) -> Obb {
    let rad = a.to_radians();
    let s = rad.sin();
    let c = rad.cos();
    let lx = w * 0.5;
    let ly = h * 0.5;
    let wx = x + lx * c - ly * s;
    let wy = y + lx * s + ly * c;
    Obb {
        pos: Vec2::new(wx, wy),
        halfs: Vec2::new(lx, ly),
        a: rad,
    }
}
