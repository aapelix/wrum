use std::time::{Duration, Instant};

use wrum_net::{
    Channel, ConnState, PlatformTransport, Transport, connect,
    proto::{ClientMsg, ServerMsg, decode_server, encode_client},
};

pub enum NetEvent {
    Connected,
    Message(ServerMsg),
    Disconnected(Option<String>),
}

const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(20);

pub struct Net {
    url: String,
    t: PlatformTransport,
    phase: ConnState,
    last_heartbeat: Instant,
}

impl Net {
    pub fn connect(url: &str) -> anyhow::Result<Net> {
        let t = connect(url)?;
        Ok(Net {
            url: url.into(),
            t,
            phase: ConnState::Connecting,
            last_heartbeat: Instant::now(),
        })
    }
    pub fn reconnect(&mut self) -> anyhow::Result<()> {
        self.t = connect(&self.url)?;
        self.phase = ConnState::Connecting;
        Ok(())
    }
    pub fn state(&self) -> ConnState {
        self.t.state()
    }
    pub fn send(&mut self, ch: Channel, msg: &ClientMsg) -> anyhow::Result<()> {
        self.t.send(ch, &encode_client(msg)?)
    }
    pub fn pump(&mut self, out: &mut Vec<NetEvent>) -> anyhow::Result<()> {
        let s = self.state();
        if s == ConnState::Open && self.last_heartbeat.elapsed() >= HEARTBEAT_INTERVAL {
            self.send(Channel::Reliable, &ClientMsg::Heartbeat)?;
            self.last_heartbeat = Instant::now();
        }
        if s == ConnState::Open && self.phase == ConnState::Connecting {
            self.phase = ConnState::Open;
            self.last_heartbeat = Instant::now();
            out.push(NetEvent::Connected);
        }
        while let Ok(Some((_, b))) = self.t.poll() {
            let msg = decode_server(&b)?;
            out.push(NetEvent::Message(msg));
        }
        if s == ConnState::Closed && self.phase != ConnState::Closed {
            self.phase = ConnState::Closed;
            out.push(NetEvent::Disconnected(self.t.error()));
        }
        Ok(())
    }
}
