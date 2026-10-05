use rkyv::{Archive, Deserialize, Serialize};

pub const INPUT_REDUNDANCY: usize = 5;

#[derive(Archive, Serialize, Deserialize, Debug)]
pub struct Input {
    seq: u32,
    steer: u8,
    thr: u8,
}

#[derive(Archive, Serialize, Deserialize, Debug)]
pub enum ClientMessage {
    Hello {
        version: u8,
        name: String,
    },
    Input {
        last_server_tick: u32,
        count: usize,
        inputs: [Input; INPUT_REDUNDANCY],
    },
}
#[derive(Archive, Serialize, Deserialize, Debug)]
pub enum ServerMessage {
    Welcome {
        your_id: u8,
        tick_rate: u8,
        // spawn: CarState,
        lobby_code: String,
    },
    Snapshot {
        // todo
    },
}
