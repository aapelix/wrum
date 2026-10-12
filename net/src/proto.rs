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
    LeaveLobby,
    Input { input: Input },
}

#[derive(Archive, Serialize, Deserialize, Debug, Clone)]
pub enum ServerMsg {
    PlayerJoined {
        id: PlayerId,
        name: String,
    },
    PlayerLeft {
        id: PlayerId,
    },
    Welcome {
        your_id: PlayerId,
    },
    Snapshot {
        tick: u32,
        cars: Vec<(PlayerId, CarState)>,
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
