use std::{
    collections::VecDeque,
    io,
    time::{Duration, Instant},
};

use shared::net::{
    Channel, ClientMessage, Config, ConnId, DisconnectReason, Endpoint, Event, SendError,
    ServerMessage,
};

#[cfg(not(target_os = "emscripten"))]
use shared::net::native::UdpTransport as Link;
#[cfg(target_os = "emscripten")]
use shared::net::web::WsClient as Link;

#[cfg(not(target_os = "emscripten"))]
pub const SERVER: &str = "127.0.0.1:7777";
#[cfg(target_os = "emscripten")]
pub const SERVER: &str = "ws://127.0.0.1:7778";

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Status {
    Connecting,
    Connected,
    Disconnected(DisconnectReason),
}

pub struct Net {
    client: Endpoint<Link, ClientMessage, ServerMessage>,
    conn: ConnId,
    status: Status,
    inbox: VecDeque<ServerMessage>,
    pub next_seq: u32,
}

impl Net {
    pub fn new(server: &str) -> io::Result<Self> {
        #[cfg(not(target_os = "emscripten"))]
        let (link, addr) = (
            Link::bind("0.0.0.0:0")?,
            server
                .parse::<std::net::SocketAddr>()
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?,
        );

        #[cfg(target_os = "emscripten")]
        let (link, addr) = (Link::connect(server)?, ());

        let mut client = Endpoint::<Link, ClientMessage, ServerMessage>::new(
            link,
            Config {
                accept: false,
                ..Config::default()
            },
        );
        let conn = client.connect(addr, Instant::now());
        Ok(Self {
            client,
            conn,
            status: Status::Connecting,
            inbox: VecDeque::new(),
            next_seq: 0,
        })
    }

    pub fn poll(&mut self) {
        let Self {
            client,
            status,
            inbox,
            ..
        } = self;
        for ev in client.poll(Instant::now()) {
            match ev {
                Event::Connected(_) => *status = Status::Connected,
                Event::Message(_, msg) => inbox.push_back(msg),
                Event::Disconnected(_, why) => *status = Status::Disconnected(why),
            }
        }
    }

    pub fn status(&self) -> Status {
        self.status
    }

    pub fn next_message(&mut self) -> Option<ServerMessage> {
        self.inbox.pop_front()
    }

    pub fn send(&mut self, ch: Channel, msg: &ClientMessage) -> Result<(), SendError> {
        self.client.send(self.conn, ch, msg)
    }

    pub fn rtt(&self) -> Option<Duration> {
        self.client.rtt(self.conn)
    }
}

impl Drop for Net {
    fn drop(&mut self) {
        self.client.disconnect(self.conn);
    }
}
