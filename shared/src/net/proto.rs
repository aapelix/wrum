use rkyv::{Archive, Deserialize, Serialize};

use crate::sim::{CarState, Input};

pub const PROTO_VERSION: u8 = 1;
pub const INPUT_REDUNDANCY: usize = 5;

#[derive(Archive, Serialize, Deserialize, Debug)]
pub enum RejectReason {
    VersionMismatch,
    ServerFull,
    LobbyNotFound,
    LobbyFull,
}

#[derive(Archive, Serialize, Deserialize, Debug)]
pub enum ClientMessage {
    Hello { version: u8, name: String },
    Quickplay,
    CreateLobby,
    JoinLobby { code: String },
    Input { inputs: Vec<Input> },
}
#[derive(Archive, Serialize, Deserialize, Debug)]
pub enum ServerMessage {
    Welcome(Welcome),
    Snapshot(Snapshot),
    Reject(RejectReason),
}

#[derive(Archive, Serialize, Deserialize, Debug)]
pub struct Snapshot {
    pub server_tick: u32,
    pub ack_seq: u32,
    pub ents: Vec<(u8, CarState)>,
}
#[derive(Archive, Serialize, Deserialize, Debug)]
pub struct Welcome {
    pub your_id: u8,
    pub tick_rate: u8,
    pub spawn: CarState,
    pub lobby_code: String,
}
