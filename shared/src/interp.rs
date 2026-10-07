use std::collections::VecDeque;

use crate::{net::proto::Snapshot, sim::CarState};

pub const SNAPSHOT_BUF_CAP: usize = 32;
pub const INTERP_DELAY_TICKS: f32 = 6.0;
pub const INTERP_SMOOTH: f32 = 0.1;
pub const INTERP_SNAP_TICKS: f32 = 20.0;

pub struct SnapshotEntry {
    pub tick: u32,
    pub entities: Vec<(u8, CarState)>,
}
impl SnapshotEntry {
    pub fn find_entity(&self, id: u8) -> Option<&CarState> {
        self.entities
            .iter()
            .find(|(e_id, _)| *e_id == id)
            .map(|(_, state)| state)
    }
}

pub struct SnapshotBuffer {
    items: VecDeque<SnapshotEntry>,
}
impl SnapshotBuffer {
    pub fn new() -> Self {
        Self {
            items: VecDeque::with_capacity(SNAPSHOT_BUF_CAP),
        }
    }

    pub fn push(&mut self, snapshot: &Snapshot) {
        if let Some(latest) = self.items.back() {
            if snapshot.server_tick <= latest.tick {
                return;
            }
        }

        self.items.push_back(SnapshotEntry {
            tick: snapshot.server_tick,
            entities: snapshot.ents.clone(),
        });

        if self.items.len() > SNAPSHOT_BUF_CAP {
            self.items.pop_front();
        }
    }
    pub fn len(&self) -> usize {
        self.items.len()
    }
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
    pub fn get(&self, index: usize) -> Option<&SnapshotEntry> {
        self.items.get(index)
    }

    pub fn latest(&self) -> Option<&SnapshotEntry> {
        self.items.back()
    }

    pub fn find_bracket(&self, render_tick: f32) -> Option<(&SnapshotEntry, &SnapshotEntry, f32)> {
        match self.items.len() {
            0 => None,

            1 => {
                let entry = &self.items[0];
                Some((entry, entry, 0.0))
            }

            n => {
                let hi = self
                    .items
                    .iter()
                    .position(|entry| entry.tick as f32 >= render_tick)
                    .unwrap_or(n - 1);

                let hi = hi.max(1);
                let lo = hi - 1;

                let a = &self.items[lo];
                let c = &self.items[hi];

                let t = if c.tick != a.tick {
                    ((render_tick - a.tick as f32) / (c.tick - a.tick) as f32).clamp(0.0, 1.0)
                } else {
                    0.0
                };

                Some((a, c, t))
            }
        }
    }
}

pub fn lerp_angle(a: f32, b: f32, t: f32) -> f32 {
    let mut d = b - a;
    while d > std::f32::consts::PI {
        d -= 2.0 * std::f32::consts::PI;
    }
    while d < -std::f32::consts::PI {
        d += 2.0 * std::f32::consts::PI;
    }
    a + d * t
}
