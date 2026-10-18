use rkyv::{Archive, Deserialize, Serialize, rancor};
use wrum_core::{
    PlayerId,
    physics::{CarState, Input},
};

pub const TICK_RATE: u32 = 60;
pub const SNAPSHOT_EVERY: u32 = 3;

#[derive(Archive, Serialize, Deserialize, Debug, Clone)]
pub enum ClientMsg {
    Hello { version: usize, name: String },
    CreateLobby,
    JoinLobby { code: String },
    JoinPublic,
    LeaveLobby,
    Input { inputs: Vec<Input> },
}

#[derive(Archive, Serialize, Deserialize, Debug, Clone)]
pub enum ServerMsg {
    PlayerJoined {
        id: PlayerId,
        name: String,
        spawn: CarState,
    },
    PlayerLeft {
        id: PlayerId,
    },
    Welcome {
        your_id: PlayerId,
        spawn: CarState,
        existing: Vec<(PlayerId, CarState)>,
        code: String,
        private: bool,
    },
    Snapshot {
        tick: u32,
        cars: Vec<(PlayerId, CarState)>,
    },
    Race {
        id: PlayerId,
        lap: u32,
        next: u32,
        finished: bool,
        lap_time: Option<f32>,
    },
    Positions {
        tick: u32,
        order: Vec<PlayerId>,
    },
    Reject(RejectReason),
}

#[derive(Archive, Serialize, Deserialize, Debug, Clone)]
pub enum RejectReason {
    LobbyFull,
    LobbyGone,
    LobbyNotFound,
    VersionMismatch,
}

pub fn decode_client(bytes: &[u8]) -> anyhow::Result<ClientMsg> {
    let deserialized = rkyv::from_bytes::<ClientMsg, rancor::Error>(bytes)?;
    Ok(deserialized)
}
pub fn decode_server(bytes: &[u8]) -> anyhow::Result<ServerMsg> {
    let deserialized = rkyv::from_bytes::<ServerMsg, rancor::Error>(bytes)?;
    Ok(deserialized)
}
pub fn encode_client(msg: &ClientMsg) -> anyhow::Result<Vec<u8>> {
    Ok(rkyv::to_bytes::<rancor::Error>(msg)?.to_vec())
}
pub fn encode_server(msg: &ServerMsg) -> anyhow::Result<Vec<u8>> {
    Ok(rkyv::to_bytes::<rancor::Error>(msg)?.to_vec())
}
