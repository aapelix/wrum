use wrum_net::{Packet, PlatformTransport, Transport, connect};

pub struct Net {
    transport: PlatformTransport,
}

impl Net {
    pub fn connect(url: &str) -> anyhow::Result<Net> {
        let transport = connect(url)?;
        Ok(Net { transport })
    }
    pub fn poll(&mut self) -> anyhow::Result<Option<Packet>> {
        self.transport.poll()
    }
}
