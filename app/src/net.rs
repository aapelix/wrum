use std::time::{Duration, Instant};

use wrum_net::{Channel, ConnState, PlatformTransport, Transport, connect};
use wrum_proto::{ClientMsg, ServerMsg, decode_server, encode_client};

pub enum NetEvent {
    Connected,
    Message(ServerMsg),
    Disconnected(Option<String>),
}

const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(10);

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
        if self.phase == ConnState::Closed {
            self.t = connect(&self.url)?;
            self.phase = ConnState::Connecting;
        }
        Ok(())
    }
    pub fn state(&self) -> ConnState {
        self.t.state()
    }
    pub fn send(&mut self, msg: &ClientMsg) -> anyhow::Result<()> {
        let ch = if msg.is_fast() {
            Channel::Unreliable
        } else {
            Channel::Reliable
        };
        self.t.send(ch, &encode_client(msg)?)
    }
    pub fn pump(&mut self, out: &mut Vec<NetEvent>) -> anyhow::Result<()> {
        let s = self.state();
        if s == ConnState::Open && self.last_heartbeat.elapsed() >= HEARTBEAT_INTERVAL {
            self.send(&ClientMsg::Heartbeat)?;
            self.last_heartbeat = Instant::now();
        }
        if s == ConnState::Open && self.phase == ConnState::Connecting {
            self.phase = ConnState::Open;
            self.last_heartbeat = Instant::now();
            out.push(NetEvent::Connected);
        }
        while let Some(b) = self.t.poll() {
            out.push(NetEvent::Message(decode_server(&b)?));
        }
        if s == ConnState::Closed && self.phase != ConnState::Closed {
            self.phase = ConnState::Closed;
            out.push(NetEvent::Disconnected(self.t.error()));
        }
        Ok(())
    }
}
