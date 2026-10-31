use glam::Vec2;

use crate::map::{PropKind, load_map};

pub enum Shape {
    Box { hw: f32, hh: f32 },
    Convex { points: Vec<Vec2> },
    Wall { points: Vec<Vec2> },
    Circle { r: f32 },
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
#[derive(Clone)]
pub struct Pad {
    pub pos: Vec2,
    pub rot: f32,
    pub dir: f32,
    pub hw: f32,
    pub hh: f32,
    pub jump_power: f32,
    // pub vel_power: f32,
}
impl Pad {
    pub fn dir(&self) -> Vec2 {
        let (s, c) = libm::sincosf(self.dir);
        Vec2::new(s, -c)
    }
    pub fn contains(&self, p: Vec2) -> bool {
        let d = p - self.pos;
        let (s, c) = libm::sincosf(-self.rot);
        let l = Vec2::new(d.x * c - d.y * s, d.x * s + d.y * c);
        l.x.abs() <= self.hw && l.y.abs() <= self.hh
    }
    pub fn corners(&self) -> [Vec2; 4] {
        let (s, c) = libm::sincosf(self.rot);
        let rotate = |x: f32, y: f32| Vec2::new(x * c - y * s, x * s + y * c) + self.pos;
        [
            rotate(-self.hw, -self.hh),
            rotate(self.hw, -self.hh),
            rotate(self.hw, self.hh),
            rotate(-self.hw, self.hh),
        ]
    }
}
#[derive(Clone)]
pub struct SpeedPad {
    pub pos: Vec2,
    pub rot: f32,
    pub hw: f32,
    pub hh: f32,
    pub level: u8,
}
impl SpeedPad {
    pub fn contains(&self, p: Vec2) -> bool {
        let d = p - self.pos;
        let (s, c) = libm::sincosf(-self.rot);
        let local = Vec2::new(d.x * c - d.y * s, d.x * s + d.y * c);

        local.x.abs() <= self.hw && local.y.abs() <= self.hh
    }
    pub fn corners(&self) -> [Vec2; 4] {
        let (s, c) = libm::sincosf(self.rot);
        let rotate = |x: f32, y: f32| Vec2::new(x * c - y * s, x * s + y * c) + self.pos;
        [
            rotate(-self.hw, -self.hh),
            rotate(self.hw, -self.hh),
            rotate(self.hw, self.hh),
            rotate(-self.hw, self.hh),
        ]
    }
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
    pub pads: Vec<Pad>,
    pub speed_pads: Vec<SpeedPad>,
}

impl Level {
    pub fn load(path: &str) -> anyhow::Result<Self> {
        let map = load_map(path)?;

        let mut obstacles = Vec::new();
        let mut spawns = Vec::new();
        let mut checkpoints = Vec::new();
        let mut pads = Vec::new();
        let mut speed_pads = Vec::new();

        for check in map.checkpoints {
            checkpoints.push(Checkpoint {
                index: check.5,
                pos: Vec2 {
                    x: check.0,
                    y: check.1,
                },
                rot: check.4.to_radians(),
                hw: check.2 / 2.,
                hh: check.3 / 2.,
            });
        }
        for spawn in map.spawns {
            spawns.push((
                Vec2 {
                    x: spawn.0,
                    y: spawn.1,
                },
                (spawn.2 + 90.).to_radians(),
            ));
        }
        for coll in map.colliders {
            obstacles.push(Obstacle {
                pos: Vec2 {
                    x: coll.0,
                    y: coll.1,
                },
                rot: coll.4.to_radians(),
                shape: Shape::Box {
                    hw: coll.2 / 2.,
                    hh: coll.3 / 2.,
                },
            });
        }
        for p in map.jump_pads {
            pads.push(Pad {
                pos: Vec2::new(p.0, p.1),
                rot: p.4.to_radians(),
                dir: (p.4 + 90.).to_radians(),
                hw: p.2 / 2.,
                hh: p.3 / 2.,
                jump_power: p.5,
            });
        }
        for p in map.speed_pads {
            speed_pads.push(SpeedPad {
                pos: Vec2::new(p.0, p.1),
                rot: p.4.to_radians(),
                hw: p.2 / 2.,
                hh: p.3 / 2.,
                level: p.5 as u8,
            });
        }
        for p in map.props {
            match p.kind {
                PropKind::Fence => {
                    let rot = p.rotation.to_radians();
                    let offset = Vec2::new(6.5, 0.0);
                    let (s, c) = libm::sincosf(rot);
                    let offset =
                        Vec2::new(offset.x * c - offset.y * s, offset.x * s + offset.y * c);

                    obstacles.push(Obstacle {
                        pos: Vec2::new(p.x, p.y) + offset,
                        rot,
                        shape: Shape::Box { hw: 8.5, hh: 2. },
                    });
                }
                PropKind::FenceEnd => {
                    obstacles.push(Obstacle {
                        pos: Vec2 { x: p.x, y: p.y },
                        rot: p.rotation.to_radians(),
                        shape: Shape::Box { hw: 2., hh: 2. },
                    });
                }
                PropKind::Tires => {
                    obstacles.push(Obstacle {
                        pos: Vec2 { x: p.x, y: p.y },
                        rot: p.rotation.to_radians(),
                        shape: Shape::Circle { r: 11. },
                    });
                }
            }
        }

        let mask_path = path.replace(".json", "_mask.png");
        let (surf_px, surf_w, surf_h) = load_mask(&mask_path)?;

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
            pads,
            speed_pads,
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
            _ => Surface::Road as u8,
        })
        .collect();
    Ok((px, w as usize, h as usize))
}
