use wrum_core::{
    bot::{Bot, BotSkill, Path},
    level::Level,
    sim::{DT, Input, MAX_CARS, V2, World, grid::Map},
};

pub struct Demo {
    pub w: World,
    prev: World,
    map: Map,
    path: Path,
    bot: Bot,
    acc: f32,
    level: Level,
}

fn wrap_pi(a: f32) -> f32 {
    use std::f32::consts::{PI, TAU};
    (a + PI).rem_euclid(TAU) - PI
}

impl Demo {
    pub fn new(level: Level) -> Self {
        let map = Map::from_level(&level);
        let path = Path::from_level(&level);
        let mut w = World::default();
        let (pos, rot) = level
            .spawns
            .first()
            .copied()
            .unwrap_or_else(|| path.pose(0));
        w.spawn(0, pos, rot);
        Self {
            prev: w,
            w,
            map,
            path,
            bot: Bot::new(0, BotSkill::NORMAL),
            acc: 0.0,
            level,
        }
    }

    pub fn level(&self) -> &Level {
        &self.level
    }

    pub fn update(&mut self, dt: f32) {
        self.acc = (self.acc + dt).min(0.25);
        while self.acc >= DT {
            self.acc -= DT;
            self.prev = self.w;
            let mut ins = [Input::default(); MAX_CARS];
            ins[0] = self.bot.think(&self.w, &self.map, &self.path);
            self.w.step(&self.map, &ins, false, &mut |_, _| {});
        }
    }

    pub fn view(&self) -> (V2, f32, f32) {
        let (a, b) = (&self.prev.cars[0], &self.w.cars[0]);
        let al = (self.acc / DT).clamp(0.0, 1.0);
        if (b.pos - a.pos).length() < 40.0 {
            (
                a.pos.lerp(b.pos, al),
                a.rot + wrap_pi(b.rot - a.rot) * al,
                a.z + (b.z - a.z) * al,
            )
        } else {
            (b.pos, b.rot, b.z)
        }
    }
}
