use rkyv::{Archive, Deserialize, Serialize};

pub const TICK_RATE: u32 = 60;
pub const SNAPSHOT_EVERY: u32 = 3;

#[derive(Archive, Serialize, Deserialize, Debug, Clone)]
pub enum ClientMsg {
    Hello { version: usize, name: String },
    CreateLobby,
    JoinLobby { code: String },
    LeaveLobby,
}

#[derive(Archive, Serialize, Deserialize, Debug, Clone)]
pub struct Input {
    pub seq: u32,
    pub steer: f32,
    pub throttle: f32,
}

#[derive(Archive, Serialize, Deserialize, Debug, Clone)]
pub enum ServerMsg {
    PlayerJoined { id: u32, name: String },
    PlayerLeft { id: u32 },
}
