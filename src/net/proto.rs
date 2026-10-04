pub struct Input {
    seq: u32,
    steer: u8,
    thr: u8,
}

pub enum ClientMessage {
    Input {
        last_server_tick: u32,
        inputs: Vec<Input>,
    },
}

pub enum ServerMessage {
    Snapshot {},
}
