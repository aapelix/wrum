use glam::Vec2;

use crate::{
    map::{LayerJson, ObjectKindJson, load_map},
    physics::Shape,
};

pub struct Obstacle {
    pub pos: Vec2,
    pub rot: f32,
    pub shape: Shape,
}

pub struct Level {
    pub obstacles: Vec<Obstacle>,
    pub spawns: Vec<(Vec2, f32)>,
}

impl Level {
    pub fn load(path: &str) -> anyhow::Result<Self> {
        let map = load_map(path)?;

        let mut obstacles = Vec::new();
        let mut spawns = Vec::new();

        for layer in &map.layers {
            match layer {
                LayerJson::ObjectLayer(layer) => {
                    for obj in &layer.objects {
                        match obj.kind {
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
                            _ => {}
                        }
                    }
                }
                _ => {}
            }
        }

        Ok(Self { obstacles, spawns })
    }
}
