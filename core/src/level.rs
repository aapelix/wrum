use glam::Vec2;

use crate::map::{LayerJson, ObjectKindJson, load_map};

pub enum Shape {
    Box { hw: f32, hh: f32 },
    Convex { points: Vec<Vec2> },
    Wall { points: Vec<Vec2> },
}

use crate::sim::Surface;

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
    pub fn contains(&self, p: Vec2) -> bool {
        let d = p - self.pos;
        let (s, c) = libm::sincosf(-self.rot);
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

pub struct Level {
    pub obstacles: Vec<Obstacle>,
    pub spawns: Vec<(Vec2, f32)>,
    pub surf_px: Vec<u8>,
    pub surf_w: usize,
    pub surf_h: usize,
    pub checkpoints: Vec<Checkpoint>,
    pub checkpoint_count: u32,
}

impl Level {
    pub fn load(path: &str) -> anyhow::Result<Self> {
        let map = load_map(path)?;

        let mut obstacles = Vec::new();
        let mut spawns = Vec::new();
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
                            ObjectKindJson::Check { properties } => {
                                let index = properties
                                    .iter()
                                    .find(|p| p.name == "index")
                                    .and_then(|p| p.value.as_u64())
                                    .ok_or_else(|| anyhow::anyhow!("checkpoint without index"))?
                                    as u32;
                                let (hw, hh) = (obj.width * 0.5, obj.height * 0.5);
                                let rot = obj.rotation.to_radians();
                                let (s, c) = libm::sincosf(rot);
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
                            _ => {}
                        }
                    }
                }
                _ => {}
            }
        }

        let (surf_px, surf_w, surf_h) = load_mask("assets/maps/untitled_mask.png")?;

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
            surf_px,
            surf_w,
            surf_h,
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
        let (x, y) = (p.x as i64, p.y as i64);
        x >= 0
            && y >= 0
            && (x as usize) < self.surf_w
            && (y as usize) < self.surf_h
            && self.surf_px[y as usize * self.surf_w + x as usize] == Surface::Grass as u8
    }
}

pub fn load_mask(path: &str) -> anyhow::Result<(Vec<u8>, usize, usize)> {
    let img = image::open(path)?.to_rgb8();
    let (w, h) = img.dimensions();
    let px = img
        .pixels()
        .map(|p| match p.0 {
            [0, 255, 0] => Surface::Grass as u8,
            [0, 0, 255] => Surface::Water as u8,
            [255, 255, 0] => Surface::Pad as u8,
            _ => Surface::Road as u8,
        })
        .collect();
    Ok((px, w as usize, h as usize))
}
