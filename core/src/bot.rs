use glam::Vec2;

use crate::{
    level::Level,
    sim::{
        CHARGE_TIERS, Input, MAX_SPEED, OFFSET, RADIUS, Status, Surface, World, axes, grid::Map,
    },
};

pub const STEP: f32 = 12.0;
const DRIFT_START_SPEED: f32 = 95.0;
const LOOK_MIN: f32 = 28.0;

pub struct Path {
    pts: Vec<Vec2>,
}

fn catmull_rom(p0: Vec2, p1: Vec2, p2: Vec2, p3: Vec2, t: f32) -> Vec2 {
    let (t2, t3) = (t * t, t * t * t);
    0.5 * (2.0 * p1
        + (p2 - p0) * t
        + (2.0 * p0 - 5.0 * p1 + 4.0 * p2 - p3) * t2
        + (3.0 * p1 - p0 - 3.0 * p2 + p3) * t3)
}

fn resample(dense: &[Vec2], step: f32) -> Vec<Vec2> {
    let mut out = vec![dense[0]];
    let mut carry = 0.0;
    for w in dense.windows(2) {
        let (a, b) = (w[0], w[1]);
        let seg = (b - a).length();
        if seg == 0.0 {
            continue;
        }
        let mut d = step - carry;
        while d <= seg {
            out.push(a + (b - a) * (d / seg));
            d += step;
        }
        carry = seg - (d - step);
    }
    out
}

impl Path {
    pub fn from_level(level: &Level) -> Self {
        let wp: Vec<Vec2> = (0..level.checkpoint_count)
            .filter_map(|i| {
                let g: Vec<_> = level.checkpoints.iter().filter(|c| c.index == i).collect();
                if g.is_empty() {
                    return None;
                }
                let sum = g.iter().fold(Vec2::ZERO, |a, c| a + c.pos);
                Some(sum / g.len() as f32)
            })
            .collect();
        Self::from_waypoints(&wp)
    }

    pub fn from_waypoints(wp: &[Vec2]) -> Self {
        assert!(wp.len() >= 3, "path needs at least 3 waypoints");
        let n = wp.len();
        let mut dense = Vec::new();
        for i in 0..n {
            let (p0, p1, p2, p3) = (wp[(i + n - 1) % n], wp[i], wp[(i + 1) % n], wp[(i + 2) % n]);
            let segs = ((p2 - p1).length() / 4.0).ceil().max(1.0) as usize;
            for k in 0..segs {
                dense.push(catmull_rom(p0, p1, p2, p3, k as f32 / segs as f32));
            }
        }
        dense.push(dense[0]);
        let mut pts = resample(&dense, STEP);
        if pts.len() > 1 && pts[pts.len() - 1].distance(pts[0]) < STEP * 0.5 {
            pts.pop();
        }
        Self { pts }
    }

    pub fn len(&self) -> usize {
        self.pts.len()
    }
    pub fn at(&self, i: usize) -> Vec2 {
        self.pts[i % self.pts.len()]
    }
    pub fn pose(&self, i: usize) -> (Vec2, f32) {
        let d = self.at(i + 1) - self.at(i);
        (self.at(i), d.x.atan2(-d.y))
    }

    pub fn nearest(&self, p: Vec2, hint: Option<usize>) -> usize {
        let n = self.pts.len();
        if let Some(h) = hint {
            let mut best = (h % n, f32::MAX);
            for k in 0..48isize {
                let i = (h as isize - 8 + k).rem_euclid(n as isize) as usize;
                let d = self.pts[i].distance_squared(p);
                if d < best.1 {
                    best = (i, d);
                }
            }
            if best.1 < 80.0 * 80.0 {
                return best.0;
            }
        }
        self.pts
            .iter()
            .enumerate()
            .min_by(|a, b| {
                a.1.distance_squared(p)
                    .partial_cmp(&b.1.distance_squared(p))
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|(i, _)| i)
            .unwrap_or(0)
    }

    pub fn bend(&self, i: usize, a: usize, b: usize) -> f32 {
        let (p0, p1, p2) = (self.at(i), self.at(i + a), self.at(i + a + b));
        let (d1, d2) = (p1 - p0, p2 - p1);
        d1.perp_dot(d2).atan2(d1.dot(d2))
    }
}

#[derive(Clone, Copy)]
pub struct BotSkill {
    pub lookahead: f32,
    pub corner_speed: f32,
    pub drift: bool,
}
impl BotSkill {
    pub const EASY: Self = Self {
        lookahead: 0.22,
        corner_speed: 0.55,
        drift: false,
    };
    pub const NORMAL: Self = Self {
        lookahead: 0.30,
        corner_speed: 0.70,
        drift: true,
    };
}

pub struct Bot {
    pub slot: usize,
    pub skill: BotSkill,
    idx: Option<usize>,
    stuck: u16,
    reverse: u16,
}

fn repulse(map: &Map, p: Vec2, range: f32) -> Vec2 {
    let mut out = Vec2::ZERO;
    for &si in map.nearby(p) {
        let d = p - map.segs[si as usize].closest(p);
        let dist = d.length();
        if dist < range && dist > 1e-3 {
            out += d / dist * (1.0 - dist / range);
        }
    }
    out
}

impl Bot {
    pub fn new(slot: usize, skill: BotSkill) -> Self {
        Self {
            slot,
            skill,
            idx: None,
            stuck: 0,
            reverse: 0,
        }
    }

    pub fn reset(&mut self) {
        self.idx = None;
        self.stuck = 0;
        self.reverse = 0;
    }

    pub fn think(&mut self, world: &World, map: &Map, path: &Path) -> Input {
        let car = &world.cars[self.slot];
        if !car.active || car.status == Status::Drowning {
            return Input::default();
        }
        if car.status == Status::Spun {
            self.stuck = 0;
            return Input::default();
        }

        let (f, r) = axes(car.rot);
        let speed = car.speed();
        let fwd = car.vel.dot(f);

        let i = path.nearest(car.pos, self.idx);
        self.idx = Some(i);
        let look = ((LOOK_MIN + speed * self.skill.lookahead) / STEP)
            .ceil()
            .max(1.0) as usize;
        let target = path.at(i + look);

        let mut to = (target - car.pos).normalize_or_zero();
        let push = repulse(map, car.pos + f * OFFSET, RADIUS + 8.0)
            + repulse(
                map,
                car.pos + f * (OFFSET + 8.0 + fwd.max(0.0) * 0.12),
                RADIUS + 6.0,
            ) * 0.5;
        to = (to + push * 1.5).normalize_or_zero();
        if to == Vec2::ZERO {
            to = f;
        }

        let ang = f.perp_dot(to).atan2(f.dot(to));
        let mut steer: i8 = if ang.abs() < 0.04 {
            0
        } else {
            ang.signum() as i8
        };

        let mut water_ahead = false;
        let ahead = car.pos + f * (24.0 + fwd.max(0.0) * 0.35);
        if map.surface_at(ahead) == Surface::Water {
            water_ahead = true;
            let right_ok = map.surface_at(ahead + r * 24.0) != Surface::Water;
            let left_ok = map.surface_at(ahead - r * 24.0) != Surface::Water;
            if right_ok {
                steer = 1;
            } else if left_ok {
                steer = -1;
            }
        }

        if self.reverse > 0 {
            self.reverse -= 1;
            return Input {
                throttle: -1,
                steer: -steer,
                drift: false,
            };
        }
        if speed < 8.0 {
            self.stuck += 1;
            if self.stuck > 45 {
                self.stuck = 0;
                self.reverse = 40;
            }
        } else {
            self.stuck = 0;
        }

        let bend = path.bend(i, look, 6);
        let sharp = (bend.abs() / 0.9).clamp(0.0, 1.0);
        let desired = MAX_SPEED * (1.0 - (1.0 - self.skill.corner_speed) * sharp);
        let mut throttle: i8 = if speed > desired + 40.0 {
            -1
        } else if speed > desired {
            0
        } else {
            1
        };
        if ang.abs() > 1.3 && speed > 70.0 {
            throttle = 0;
        }
        if water_ahead && speed > 100.0 {
            throttle = 0;
        }

        let mut drift = false;
        if self.skill.drift && !water_ahead {
            if car.drift_dir != 0 {
                let want_release = bend.abs() < 0.2 && car.tier() > 0
                    || bend.abs() < 0.08
                    || car.charge >= CHARGE_TIERS[2] && bend.abs() < 0.3;
                drift = !want_release;
            } else if bend.abs() > 0.6 && fwd > DRIFT_START_SPEED && car.z <= 0.0 {
                drift = true;
                steer = bend.signum() as i8;
            }
            if drift {
                throttle = 1;
            }
        }

        Input {
            throttle,
            steer,
            drift,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn circle_path_is_closed_and_evenly_spaced() {
        let wp: Vec<Vec2> = (0..12)
            .map(|k| {
                let a = k as f32 / 12.0 * std::f32::consts::TAU;
                Vec2::new(a.cos(), a.sin()) * 300.0 + Vec2::splat(400.0)
            })
            .collect();
        let p = Path::from_waypoints(&wp);
        assert!(p.len() > 100);
        for i in 0..p.len() {
            let d = p.at(i).distance(p.at(i + 1));
            assert!((d - STEP).abs() < 1.5, "gap {d} at {i}");
        }
        assert_eq!(p.nearest(p.at(10) + Vec2::new(1.0, 1.0), Some(8)), 10);
    }

    #[test]
    #[ignore]
    fn bot_finishes_a_lap() {
        use crate::{race::RaceProgress, sim::MAX_CARS};
        let path = std::env::var("WRUM_TEST_LEVEL").unwrap();
        let level = Level::load(&path).unwrap();
        let map = Map::from_level(&level);
        let line = Path::from_level(&level);
        let mut world = World::default();
        let (pos, rot) = level.spawns[0];
        world.spawn(0, pos, rot);
        let mut bot = Bot::new(0, BotSkill::NORMAL);
        let mut prog = RaceProgress::new(0);
        let n = level.checkpoint_count;
        for _ in 0..60 * 120 {
            let mut ins = [Input::default(); MAX_CARS];
            ins[0] = bot.think(&world, &map, &line);
            let mut crossed = Vec::new();
            world.step(&map, &ins, false, &mut |i, g| {
                if i == 0 {
                    crossed.push(g)
                }
            });
            prog.update(&crossed, n, world.tick);
            if prog.lap > 1 {
                return;
            }
        }
        panic!(
            "bot did not complete a lap in 120s (next checkpoint {})",
            prog.next
        );
    }
}
