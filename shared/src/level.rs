use glam::Vec2;

use crate::{
    coll::{Obb, Rect},
    map::{LayerJson, ObjectKindJson, load_map},
    sim::{CarState, Input, SurfaceMods},
};

pub const WALL_RESTITUTION: f32 = 0.4;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Surface {
    Road,
    Grass,
    Water,
}

#[derive(Clone)]
pub struct Level {
    walls: Vec<Obb>,
    bridges: Vec<Rect>,
    water: Vec<Rect>,
}

impl Level {
    pub fn load(path: &str) -> Self {
        let Ok(map) = load_map(path) else {
            return Self {
                walls: Vec::new(),
                bridges: Vec::new(),
                water: Vec::new(),
            };
        };

        let mut walls = Vec::new();
        let mut bridges = Vec::new();
        let mut water = Vec::new();

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
                            ObjectKindJson::Bridge => {
                                bridges.push(Rect {
                                    pos: Vec2 { x: obj.x, y: obj.y },
                                    size: Vec2 {
                                        x: obj.width,
                                        y: obj.height,
                                    },
                                });
                            }
                            ObjectKindJson::WaterColl => {
                                water.push(Rect {
                                    pos: Vec2 { x: obj.x, y: obj.y },
                                    size: Vec2 {
                                        x: obj.width,
                                        y: obj.height,
                                    },
                                });
                            }
                            _ => {}
                        }
                    }
                }
                _ => {}
            }
        }

        Self {
            walls,
            bridges,
            water,
        }
    }
    pub fn resolve_car(&self, car: &mut CarState) {
        for wall in &self.walls {
            car.resolve_vs_obstacle(*wall, WALL_RESTITUTION);
        }
    }
    pub fn surface_at(&self, p: Vec2) -> Surface {
        if self.bridges.iter().any(|r| r.contains(p)) {
            return Surface::Road;
        }
        if self.water.iter().any(|r| r.contains(p)) {
            return Surface::Water;
        }
        return Surface::Road;
        //self.ground_at(p)
    }
    pub fn step_car(&self, car: &mut CarState, input: &Input) {
        if car.sink > 0 {
            car.step_sinking();
            return;
        }

        let surf = self.surface_at(car.pos);
        car.step(input, SurfaceMods::of(surf));
        self.resolve_car(car);

        match self.surface_at(car.pos) {
            Surface::Water => {
                car.sink = 1;
                car.vz = 0.;
            }
            _ => self.update_safe_point(car),
        }
    }
    fn update_safe_point(&self, car: &mut CarState) {
        const M: f32 = 24.;
        let near_water = [(M, 0.), (-M, 0.), (0., M), (0., -M)]
            .iter()
            .any(|(dx, dy)| {
                self.surface_at(Vec2 {
                    x: car.pos.x + dx,
                    y: car.pos.y + dy,
                }) == Surface::Water
            });
        if !near_water {
            car.safe = car.pos;
            car.safe_a = car.a;
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
