use rkyv::{Archive, Deserialize, Serialize};

use crate::sim::{Input, MAX_CARS, World, grid::Map};

pub const REPLAY_FORMAT_VERSION: u16 = 1;
pub const REPLAY_SIM_VERSION: u16 = 1;

pub const MAX_REPLAY_TICKS: u32 = 60 * 60 * 10;

#[derive(Archive, Serialize, Deserialize, Clone, Debug)]
pub struct ReplayPlayer {
    pub slot: u8,
    pub name: String,
    pub country: [char; 2],
    pub finish_tick: Option<u32>,
}

#[derive(Archive, Serialize, Deserialize, Clone, Copy, Debug)]
pub struct ReplayRemoval {
    pub tick: u32,
    pub slot: u8,
}

#[derive(Archive, Serialize, Deserialize, Clone, Debug)]
pub struct Replay {
    pub format_version: u16,
    pub sim_version: u16,
    pub level: String,

    pub start: World,
    pub players: Vec<ReplayPlayer>,

    pub ticks: u32,

    pub inputs: Vec<u8>,

    pub removed: Vec<ReplayRemoval>,
}

#[derive(Debug)]
pub enum ReplayError {
    Serialize(String),
    Deserialize(String),
    UnsupportedFormat(u16),
    UnsupportedSimulation(u16),
    InvalidLength,
    TooLong,
    InvalidRemoval,
}

impl Replay {
    pub fn encode(&self) -> Result<Vec<u8>, ReplayError> {
        let bytes = rkyv::to_bytes::<rkyv::rancor::Error>(self)
            .map_err(|e| ReplayError::Serialize(e.to_string()))?;

        Ok(bytes.to_vec())
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, ReplayError> {
        let replay: Self = rkyv::from_bytes::<Self, rkyv::rancor::Error>(bytes)
            .map_err(|e| ReplayError::Deserialize(e.to_string()))?;

        replay.validate()?;
        Ok(replay)
    }

    pub fn validate(&self) -> Result<(), ReplayError> {
        if self.format_version != REPLAY_FORMAT_VERSION {
            return Err(ReplayError::UnsupportedFormat(self.format_version));
        }

        if self.sim_version != REPLAY_SIM_VERSION {
            return Err(ReplayError::UnsupportedSimulation(self.sim_version));
        }

        if self.ticks > MAX_REPLAY_TICKS {
            return Err(ReplayError::TooLong);
        }

        let expected = self.ticks as usize * MAX_CARS;
        if self.inputs.len() != expected {
            return Err(ReplayError::InvalidLength);
        }

        let mut previous_tick = 0;
        for (i, event) in self.removed.iter().enumerate() {
            if event.slot as usize >= MAX_CARS || event.tick >= self.ticks {
                return Err(ReplayError::InvalidRemoval);
            }

            if i > 0 && event.tick < previous_tick {
                return Err(ReplayError::InvalidRemoval);
            }

            previous_tick = event.tick;
        }

        Ok(())
    }
}

fn encode_input(input: Input) -> u8 {
    fn axis(value: i8) -> u8 {
        match value.clamp(-1, 1) {
            -1 => 0,
            0 => 1,
            1 => 2,
            _ => unreachable!(),
        }
    }

    axis(input.throttle) | (axis(input.steer) << 2) | ((input.drift as u8) << 4)
}

fn decode_input(byte: u8) -> Input {
    fn axis(value: u8) -> i8 {
        match value & 0b11 {
            0 => -1,
            1 => 0,
            2 => 1,
            _ => 0,
        }
    }

    Input {
        throttle: axis(byte),
        steer: axis(byte >> 2),
        drift: (byte & (1 << 4)) != 0,
    }
}

pub struct ReplayRecorder {
    level: String,
    start: World,
    players: Vec<ReplayPlayer>,
    ticks: u32,
    inputs: Vec<u8>,
    removed: Vec<ReplayRemoval>,
}

impl ReplayRecorder {
    pub fn new(level: String, start: World, players: Vec<ReplayPlayer>) -> Self {
        Self {
            level,
            start,
            players,
            ticks: 0,
            inputs: Vec::new(),
            removed: Vec::new(),
        }
    }
    pub fn push(&mut self, inputs: &[Input; MAX_CARS]) {
        assert!(self.ticks < MAX_REPLAY_TICKS);

        self.inputs.extend(inputs.iter().copied().map(encode_input));

        self.ticks += 1;
    }
    pub fn remove(&mut self, slot: u8) {
        if slot as usize >= MAX_CARS {
            return;
        }

        self.removed.push(ReplayRemoval {
            tick: self.ticks,
            slot,
        });
    }
    pub fn finish(&mut self, slot: u8) {
        if let Some(player) = self.players.iter_mut().find(|p| p.slot == slot) {
            player.finish_tick.get_or_insert(self.ticks);
        }
    }
    pub fn snapshot_for(&self, slot: u8) -> Option<Replay> {
        let player = self.players.iter().find(|p| p.slot == slot)?;
        let ticks = player.finish_tick?;

        if ticks > self.ticks {
            return None;
        }

        let input_len = ticks as usize * MAX_CARS;

        let players = self
            .players
            .iter()
            .cloned()
            .map(|mut p| {
                p.finish_tick = p.finish_tick.filter(|&t| t <= ticks);
                p
            })
            .collect();

        Some(Replay {
            format_version: REPLAY_FORMAT_VERSION,
            sim_version: REPLAY_SIM_VERSION,
            level: self.level.clone(),
            start: self.start,
            players,
            ticks,
            inputs: self.inputs[..input_len].to_vec(),
            removed: self
                .removed
                .iter()
                .copied()
                .filter(|event| event.tick < ticks)
                .collect(),
        })
    }
}

pub struct ReplayPlayback {
    replay: Replay,
    world: World,
    tick: u32,
    next_removal: usize,
}

impl ReplayPlayback {
    pub fn new(replay: Replay) -> Result<Self, ReplayError> {
        replay.validate()?;

        let world = replay.start;

        Ok(Self {
            replay,
            world,
            tick: 0,
            next_removal: 0,
        })
    }

    pub fn world(&self) -> &World {
        &self.world
    }
    pub fn tick(&self) -> u32 {
        self.tick
    }
    pub fn finished(&self) -> bool {
        self.tick >= self.replay.ticks
    }
    pub fn step(&mut self, map: &Map) -> bool {
        if self.finished() {
            return false;
        }

        while let Some(event) = self.replay.removed.get(self.next_removal) {
            if event.tick != self.tick {
                break;
            }

            self.world.remove(event.slot as usize);
            self.next_removal += 1;
        }

        let base = self.tick as usize * MAX_CARS;
        let inputs: [Input; MAX_CARS] =
            std::array::from_fn(|i| decode_input(self.replay.inputs[base + i]));

        self.world.step(map, &inputs, true, &mut |_, _| {});

        self.tick += 1;
        true
    }
    pub fn player(&self, slot: u8) -> Option<&ReplayPlayer> {
        self.replay.players.iter().find(|p| p.slot == slot)
    }
}
