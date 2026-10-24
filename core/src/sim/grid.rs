use glam::Vec2;

use crate::{
    level::{Level, Shape},
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
        for z in &level.zones {
            grow(&xform(z.pos, z.rot, &z.points));
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

        let (cell, gcell) = (4.0f32, 32.0f32);
        let (w, h) = (hi.x + 64.0, hi.y + 64.0);
        let (sw, sh) = ((w / cell).ceil() as usize, (h / cell).ceil() as usize);
        let (gw, gh) = ((w / gcell).ceil() as usize, (h / gcell).ceil() as usize);

        let mut surf = vec![0u8; sw * sh];
        for z in &level.zones {
            let poly = xform(z.pos, z.rot, &z.points);
            let (mut lo, mut up) = (poly[0], poly[0]);
            for p in &poly {
                lo = lo.min(*p);
                up = up.max(*p);
            }
            let (x0, y0) = (
                (lo.x / cell).max(0.0) as usize,
                (lo.y / cell).max(0.0) as usize,
            );
            let (x1, y1) = (
                ((up.x / cell) as usize).min(sw - 1),
                ((up.y / cell) as usize).min(sh - 1),
            );
            for y in y0..=y1 {
                for x in x0..=x1 {
                    let c = Vec2::new((x as f32 + 0.5) * cell, (y as f32 + 0.5) * cell);
                    if z.contains(c) {
                        surf[y * sw + x] = z.surface as u8;
                    }
                }
            }
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
