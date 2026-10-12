use wrum_net::{
    Channel, ConnState, PlatformTransport, Transport, connect,
    proto::{ClientMsg, ServerMsg, decode_server, encode_client},
};

pub enum NetEvent {
    Connected,
    Message(ServerMsg),
    Disconnected(Option<String>),
}

pub struct Net {
    url: String,
    t: PlatformTransport,
    phase: ConnState,
}

impl Net {
    pub fn connect(url: &str) -> anyhow::Result<Net> {
        let t = connect(url)?;
        Ok(Net {
            url: url.into(),
            t,
            phase: ConnState::Connecting,
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
        let s = self.t.state();
        if s == ConnState::Open && self.phase == ConnState::Connecting {
            self.phase = ConnState::Open;
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
