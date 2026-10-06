use rkyv::{Archive, Deserialize, Serialize};

pub const TICK_RATE: u8 = 60;

#[derive(Archive, Serialize, Deserialize, Debug, Default)]
pub struct Input {
    pub seq: u32,
    pub steer: u8,
    pub thr: u8,
}

#[derive(Archive, Serialize, Deserialize, Debug, Default, Clone, Copy)]
pub struct CarState {
    pub x: f32,
    pub y: f32,
    pub a: f32,
    pub s: f32,
    pub t: f32,
}
