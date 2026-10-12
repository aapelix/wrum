use glam::Vec2;

use crate::{
    map::{LayerJson, ObjectKindJson, load_map},
    physics::{Shape, Surface},
};

const TILE_SIZE: f32 = 16.;

pub struct Obstacle {
    pub pos: Vec2,
    pub rot: f32,
    pub shape: Shape,
}
pub struct Zone {
    pub pos: Vec2,
    pub rot: f32,
    pub points: Vec<Vec2>, // relative to pos
    pub surface: Surface,
}
pub struct Level {
    pub obstacles: Vec<Obstacle>,
    pub spawns: Vec<(Vec2, f32)>,
    pub zones: Vec<Zone>,
}

impl Level {
    pub fn load(path: &str) -> anyhow::Result<Self> {
        let map = load_map(path)?;

        let mut obstacles = Vec::new();
        let mut spawns = Vec::new();
        let mut zones = Vec::new();

        for layer in &map.layers {
            match layer {
                LayerJson::ObjectLayer(layer) => {
                    for obj in &layer.objects {
                        match &obj.kind {
                            ObjectKindJson::Wall => {
                                let hw = obj.width * 0.5;
                                let hh = obj.height * 0.5;

                                obstacles.push(Obstacle {
                                    pos: Vec2 {
                                        x: obj.x + hw,
                                        y: obj.y + hh,
                                    },
                                    rot: obj.rotation.to_radians(),
                                    shape: Shape::Box { hw, hh },
                                });
                            }
                            ObjectKindJson::Spawn => {
                                spawns
                                    .push((Vec2 { x: obj.x, y: obj.y }, obj.rotation.to_radians()));
                            }
                            ObjectKindJson::Grass { polygon } => {
                                zones.push(Zone {
                                    pos: Vec2 { x: obj.x, y: obj.y },
                                    rot: obj.rotation.to_radians(),
                                    points: polygon.iter().map(|p| Vec2::new(p.x, p.y)).collect(),
                                    surface: Surface::Grass,
                                });
                            }
                            ObjectKindJson::Road { polygon } => {
                                zones.push(Zone {
                                    pos: Vec2 { x: obj.x, y: obj.y },
                                    rot: obj.rotation.to_radians(),
                                    points: polygon.iter().map(|p| Vec2::new(p.x, p.y)).collect(),
                                    surface: Surface::Road,
                                });
                            }
                            _ => {}
                        }
                    }
                }
                _ => {}
            }
        }

        Ok(Self {
            obstacles,
            spawns,
            zones,
        })
    }
}
