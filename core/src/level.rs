use glam::Vec2;

use crate::{
    map::{LayerJson, ObjectKindJson, load_map},
    physics::{Shape, Surface},
};

pub struct Checkpoint {
    pub index: u32,
    pub pos: Vec2,
    pub rot: f32,
    pub hw: f32,
    pub hh: f32,
}
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
impl Zone {
    fn contains(&self, p: Vec2) -> bool {
        let d = p - self.pos;
        let (s, c) = (-self.rot).sin_cos();
        let l = Vec2::new(d.x * c - d.y * s, d.x * s + d.y * c);
        let pts = &self.points;
        let mut inside = false;
        let mut j = pts.len() - 1;
        for i in 0..pts.len() {
            let (a, b) = (pts[i], pts[j]);
            if (a.y > l.y) != (b.y > l.y) && l.x < (b.x - a.x) * (l.y - a.y) / (b.y - a.y) + a.x {
                inside = !inside;
            }
            j = i;
        }
        inside
    }
}

#[derive(Default)]
pub struct Level {
    pub obstacles: Vec<Obstacle>,
    pub spawns: Vec<(Vec2, f32)>,
    pub zones: Vec<Zone>,
    pub checkpoints: Vec<Checkpoint>,
    pub checkpoint_count: u32,
}

impl Level {
    pub fn load(path: &str) -> anyhow::Result<Self> {
        let map = load_map(path)?;

        let mut obstacles = Vec::new();
        let mut spawns = Vec::new();
        let mut zones = Vec::new();
        let mut checkpoints = Vec::new();

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
                            ObjectKindJson::Spawn { properties } => {
                                let rotation = properties
                                    .iter()
                                    .find(|p| p.name == "rotation")
                                    .and_then(|p| p.value.as_f64())
                                    .unwrap() as f32;
                                spawns.push((Vec2 { x: obj.x, y: obj.y }, rotation.to_radians()));
                            }
                            ObjectKindJson::Grass { polygon } => {
                                zones.push(Zone {
                                    pos: Vec2 { x: obj.x, y: obj.y },
                                    rot: obj.rotation.to_radians(),
                                    points: polygon.iter().map(|p| Vec2::new(p.x, p.y)).collect(),
                                    surface: Surface::Grass,
                                });
                            }
                            ObjectKindJson::WaterColl { polygon } => {
                                zones.push(Zone {
                                    pos: Vec2 { x: obj.x, y: obj.y },
                                    rot: obj.rotation.to_radians(),
                                    points: polygon.iter().map(|p| Vec2::new(p.x, p.y)).collect(),
                                    surface: Surface::Water,
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
                            ObjectKindJson::Check { properties } => {
                                let index = properties
                                    .iter()
                                    .find(|p| p.name == "index")
                                    .and_then(|p| p.value.as_u64())
                                    .ok_or_else(|| anyhow::anyhow!("checkpoint without index"))?
                                    as u32;
                                let (hw, hh) = (obj.width * 0.5, obj.height * 0.5);
                                let rot = obj.rotation.to_radians();
                                let (s, c) = rot.sin_cos();
                                let center =
                                    Vec2::new(obj.x + hw * c - hh * s, obj.y + hw * s + hh * c);
                                checkpoints.push(Checkpoint {
                                    index,
                                    pos: center,
                                    rot,
                                    hw,
                                    hh,
                                });
                            }
                            ObjectKindJson::JumpPadColl => {
                                zones.push(Zone {
                                    pos: Vec2 { x: obj.x, y: obj.y },
                                    rot: obj.rotation.to_radians(),
                                    points: vec![
                                        Vec2::new(0., 0.),
                                        Vec2::new(obj.width, 0.),
                                        Vec2::new(obj.width, obj.height),
                                        Vec2::new(0., obj.height),
                                    ],
                                    surface: Surface::Pad,
                                });
                            }
                            _ => {}
                        }
                    }
                }
                _ => {}
            }
        }

        let checkpoint_count = checkpoints
            .iter()
            .map(|c| c.index)
            .max()
            .map_or(0, |m| m + 1);
        anyhow::ensure!(checkpoint_count >= 2, "need at least 2 checkpoint indices");
        for i in 0..checkpoint_count {
            anyhow::ensure!(
                checkpoints.iter().any(|c| c.index == i),
                "no checkpoint with index {i}"
            );
        }

        Ok(Self {
            obstacles,
            spawns,
            zones,
            checkpoints,
            checkpoint_count,
        })
    }
    pub fn dist_to_gate(&self, index: u32, p: Vec2) -> f32 {
        self.checkpoints
            .iter()
            .filter(|c| c.index == index)
            .map(|c| c.pos.distance(p))
            .fold(f32::MAX, f32::min)
    }
    pub fn on_grass(&self, p: Vec2) -> bool {
        self.zones
            .iter()
            .any(|z| z.surface == Surface::Grass && z.contains(p))
    }
}
