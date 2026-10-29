use rkyv::{Archive, Deserialize, Serialize, rancor, util::AlignedVec};
use rustrict::CensorStr;
use wrum_core::{
    PlayerId,
    sim::{Input, World},
};

pub const PROTO_VERSION: usize = 4;

pub const TICK_RATE: u32 = 60;
pub const SNAPSHOT_EVERY: u32 = 2;

#[derive(Archive, Serialize, Deserialize, Debug, Clone)]
pub enum ClientMsg {
    Hello {
        version: usize,
        token: Option<String>,
    }, // None = guest
    Register {
        country: [char; 2],
        name: String,
    },
    Update {
        country: [char; 2],
        name: String,
    },
    CreateLobby,
    JoinLobby {
        code: String,
    },
    JoinPublic,
    LeaveLobby,
    Ready(bool),
    StartRace,
    PlayAgain,
    Heartbeat,
    Inputs {
        first: u32,
        inputs: Vec<Input>,
    },
    Ping {
        t0: u32,
    },
    GetLeaderboard {
        kind: BoardKind,
    },
}
#[derive(Archive, Serialize, Deserialize, Debug, Clone)]
pub enum ServerMsg {
    HelloOk {
        name: String,
        country: [char; 2],
        guest: bool,
    },
    Welcome {
        slot: PlayerId,
        tick: u32,
        code: String,
        private: bool,
        players: Vec<(PlayerId, String, [char; 2])>,
    },
    PlayerJoined {
        slot: PlayerId,
        name: String,
        country: [char; 2],
    },
    PlayerLeft {
        slot: PlayerId,
    },
    Snapshot {
        world: World,
        early: i8,
    },
    Relay {
        slot: PlayerId,
        first: u32,
        inputs: Vec<Input>,
    },
    Pong {
        t0: u32,
        tick: u32,
        frac: f32,
    },
    Race {
        slot: PlayerId,
        lap: u32,
        next: u32,
        finished: bool,
        lap_time: Option<f32>,
    },
    Positions {
        order: Vec<PlayerId>,
    },
    Reject(RejectReason),
    LobbyState {
        host: PlayerId,
        ready: Vec<PlayerId>,
    },
    Countdown {
        go_tick: u32,
    },
    Results {
        order: Vec<PlayerId>,
        players: Vec<PlayerResult>,
        reset_tick: u32,
    },
    RaceReset,
    Registered {
        token: String,
        name: String,
        country: [char; 2],
    },
    Updated {
        name: String,
        country: [char; 2],
    },
    NameRejected(NameError),
    Leaderboard {
        kind: BoardKind,
        rows: Vec<(String, f32, [char; 2])>,
        mine: Option<(u32, f32)>,
    },
}

#[derive(Archive, Serialize, Deserialize, Debug, Clone)]
pub struct PlayerResult {
    pub id: PlayerId,
    pub lap_times: Vec<f32>,
    pub total_time: f32,
    pub finished: bool,
}

#[derive(Archive, Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum RejectReason {
    LobbyFull,
    LobbyGone,
    LobbyNotFound,
    VersionMismatch,
    RaceStarted,
}
#[derive(Archive, Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum NameError {
    Taken,
    Invalid,
    Busy,
}

#[derive(Archive, Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoardKind {
    Lap,
    Total,
}

fn aligned(b: &[u8]) -> AlignedVec<16> {
    let mut v = AlignedVec::<16>::with_capacity(b.len());
    v.extend_from_slice(b);
    v
}
pub fn decode_client(b: &[u8]) -> anyhow::Result<ClientMsg> {
    Ok(rkyv::from_bytes::<ClientMsg, rancor::Error>(&aligned(b))?)
}
pub fn decode_server(b: &[u8]) -> anyhow::Result<ServerMsg> {
    Ok(rkyv::from_bytes::<ServerMsg, rancor::Error>(&aligned(b))?)
}
pub fn encode_client(m: &ClientMsg) -> anyhow::Result<Vec<u8>> {
    Ok(rkyv::to_bytes::<rancor::Error>(m)?.to_vec())
}
pub fn encode_server(m: &ServerMsg) -> anyhow::Result<Vec<u8>> {
    Ok(rkyv::to_bytes::<rancor::Error>(m)?.to_vec())
}

pub fn valid_name(raw: &str) -> Option<String> {
    let n = raw.trim();
    let ok = (3..=16).contains(&n.chars().count())
        && n.chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | ' '))
        && !n.contains("  ");
    let lower = n.to_lowercase();
    (ok && !lower.is_inappropriate()).then(|| n.to_string())
}
