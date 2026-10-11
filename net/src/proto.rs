use rkyv::{Archive, Deserialize, Serialize, rancor};

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
