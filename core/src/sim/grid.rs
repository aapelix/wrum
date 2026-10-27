use glam::Vec2;

use crate::{
    level::{Level, Pad, Shape, SpeedPad},
    sim::Surface,
};

#[derive(Clone, Copy)]
pub struct Seg {
    pub a: Vec2,
    pub b: Vec2,
}
impl Seg {
    pub fn closest(&self, p: Vec2) -> Vec2 {
        let d = self.b - self.a;
        let l2 = d.length_squared();
        if l2 == 0.0 {
            return self.a;
        }
        self.a + d * ((p - self.a).dot(d) / l2).clamp(0.0, 1.0)
    }
}

pub struct Gate {
    pub a: Vec2,
    pub b: Vec2,
    pub index: u32,
}

pub fn crossed(g: &Gate, p0: Vec2, p1: Vec2) -> bool {
    let e = g.b - g.a;
    let s0 = e.perp_dot(p0 - g.a);
    let s1 = e.perp_dot(p1 - g.a);
    if (s0 <= 0.0) == (s1 <= 0.0) {
        return false;
    }
    let t = s0 / (s0 - s1);
    let hit = p0 + (p1 - p0) * t;
    (0.0..=1.0).contains(&((hit - g.a).dot(e) / e.dot(e)))
}

pub struct Map {
    pub cell: f32,
    pub sw: usize,
    pub sh: usize,
    pub surf: Vec<u8>,
    gcell: f32,
    gw: usize,
    gh: usize,
    grid: Vec<Vec<u16>>,
    pub segs: Vec<Seg>,
    pub gates: Vec<Gate>,
    pub pads: Vec<Pad>,
    pub speed_pads: Vec<SpeedPad>,
}

fn xform(pos: Vec2, rot: f32, pts: &[Vec2]) -> Vec<Vec2> {
    let (s, c) = libm::sincosf(rot);
    pts.iter()
        .map(|p| Vec2::new(p.x * c - p.y * s, p.x * s + p.y * c) + pos)
        .collect()
}

impl Map {
    pub fn from_level(level: &Level) -> Self {
        let mut segs = Vec::new();
        let mut hi = Vec2::ZERO;
        hi = hi.max(Vec2::new(level.surf_w as f32, level.surf_h as f32));
        let mut grow = |pts: &[Vec2]| {
            for p in pts {
                hi = hi.max(*p);
            }
        };

        for o in &level.obstacles {
            let (pts, closed) = match &o.shape {
                Shape::Box { hw, hh } => (
                    vec![
                        Vec2::new(-hw, -hh),
                        Vec2::new(*hw, -hh),
                        Vec2::new(*hw, *hh),
                        Vec2::new(-hw, *hh),
                    ],
                    true,
                ),
                Shape::Convex { points } => (points.clone(), true),
                Shape::Wall { points } => (points.clone(), false),
            };
            let w = xform(o.pos, o.rot, &pts);
            grow(&w);
            for i in 0..w.len().saturating_sub(1) {
                segs.push(Seg {
                    a: w[i],
                    b: w[i + 1],
                });
            }
            if closed && w.len() > 2 {
                segs.push(Seg {
                    a: w[w.len() - 1],
                    b: w[0],
                });
            }
        }

        let gates: Vec<Gate> = level
            .checkpoints
            .iter()
            .map(|c| {
                let h = if c.hh > c.hw {
                    Vec2::new(0.0, c.hh)
                } else {
                    Vec2::new(c.hw, 0.0)
                };
                let h = xform(Vec2::ZERO, c.rot, &[h])[0];
                grow(&[c.pos + h, c.pos - h]);
                Gate {
                    a: c.pos - h,
                    b: c.pos + h,
                    index: c.index,
                }
            })
            .collect();

        let (cell, gcell) = (1.0f32, 32.0f32);
        let (w, h) = (hi.x + 64.0, hi.y + 64.0);
        let (sw, sh) = ((w / cell).ceil() as usize, (h / cell).ceil() as usize);
        let (gw, gh) = ((w / gcell).ceil() as usize, (h / gcell).ceil() as usize);

        let mut surf = vec![Surface::Road as u8; sw * sh];
        for y in 0..level.surf_h.min(sh) {
            let n = level.surf_w.min(sw);
            surf[y * sw..y * sw + n]
                .copy_from_slice(&level.surf_px[y * level.surf_w..y * level.surf_w + n]);
        }

        let mut grid = vec![Vec::new(); gw * gh];
        let gc = |v: f32, n: usize| ((v / gcell).max(0.0) as usize).min(n - 1);
        for (i, s) in segs.iter().enumerate() {
            let (lo, up) = (
                s.a.min(s.b) - Vec2::splat(4.0),
                s.a.max(s.b) + Vec2::splat(4.0),
            );
            for y in gc(lo.y, gh)..=gc(up.y, gh) {
                for x in gc(lo.x, gw)..=gc(up.x, gw) {
                    grid[y * gw + x].push(i as u16);
                }
            }
        }
        Self {
            cell,
            sw,
            sh,
            surf,
            gcell,
            gw,
            gh,
            grid,
            segs,
            gates,
            pads: level.pads.clone(),
            speed_pads: level.speed_pads.clone(),
        }
    }
    pub fn surface_at(&self, p: Vec2) -> Surface {
        if p.x < 0.0 || p.y < 0.0 {
            return Surface::Road;
        }
        let (x, y) = ((p.x / self.cell) as usize, (p.y / self.cell) as usize);
        if x >= self.sw || y >= self.sh {
            return Surface::Road;
        }
        Surface::from_u8(self.surf[y * self.sw + x])
    }
    pub fn pad_at(&self, p: Vec2) -> Option<(f32, Vec2)> {
        self.pads
            .iter()
            .find(|pad| pad.contains(p))
            .map(|pad| (pad.jump_power, pad.dir()))
    }
    pub fn speed_pad_at(&self, p: Vec2) -> Option<u8> {
        self.speed_pads
            .iter()
            .find(|pad| pad.contains(p))
            .map(|pad| pad.level.clamp(1, 3))
    }
    pub fn nearby(&self, p: Vec2) -> &[u16] {
        if p.x < 0.0 || p.y < 0.0 {
            return &[];
        }
        let (x, y) = ((p.x / self.gcell) as usize, (p.y / self.gcell) as usize);
        if x >= self.gw || y >= self.gh {
            return &[];
        }
        &self.grid[y * self.gw + x]
    }
}
