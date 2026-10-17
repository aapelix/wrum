use crate::TICK_DT;

pub const TOTAL_LAPS: u32 = 3;

pub struct RaceProgress {
    pub next: u32,
    pub lap: u32,
    pub finished: bool,
    lap_start_tick: u32,
}

pub enum RaceEvent {
    Checkpoint,
    Lap { lap_time: f32 },
    Finished { lap_time: f32 },
}

impl RaceProgress {
    pub fn new(tick: u32) -> Self {
        Self {
            next: 1,
            lap: 1,
            finished: false,
            lap_start_tick: tick,
        }
    }
    pub fn update(&mut self, touching: &[u32], n: u32, tick: u32) -> Option<RaceEvent> {
        if self.finished || !touching.contains(&self.next) {
            return None;
        }

        let crossed_line = self.next == 0;
        self.next = (self.next + 1) % n;
        if !crossed_line {
            return Some(RaceEvent::Checkpoint);
        }

        let lap_time = tick.wrapping_sub(self.lap_start_tick) as f32 * TICK_DT;
        self.lap_start_tick = tick;
        if self.lap >= TOTAL_LAPS {
            self.finished = true;
            Some(RaceEvent::Finished { lap_time })
        } else {
            self.lap += 1;
            Some(RaceEvent::Lap { lap_time })
        }
    }
}
