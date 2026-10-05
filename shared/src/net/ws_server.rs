use std::{
    collections::{HashMap, VecDeque},
    io,
    net::{SocketAddr, TcpListener, TcpStream, ToSocketAddrs},
};

use tungstenite::{
    HandshakeError, Message, WebSocket,
    handshake::{
        MidHandshake,
        server::{NoCallback, ServerHandshake},
    },
};

use crate::net::Transport;

type Mid = MidHandshake<ServerHandshake<TcpStream, NoCallback>>;

const MAX_PENDING_HANDSHAKES: usize = 256;
const MAX_INBOX: usize = 8192;
pub const WS_MTU: usize = 64 * 1024;

pub struct WsServer {
    listener: TcpListener,
    pending: Vec<Mid>,
    clients: HashMap<u64, WebSocket<TcpStream>>,
    inbox: VecDeque<(u64, Vec<u8>)>,
    closed: VecDeque<u64>,
    next_id: u64,
}

impl WsServer {
    pub fn bind(addr: impl ToSocketAddrs) -> io::Result<Self> {
        let listener = TcpListener::bind(addr)?;
        listener.set_nonblocking(true)?;
        Ok(Self {
            listener,
            pending: Vec::new(),
            clients: HashMap::new(),
            inbox: VecDeque::new(),
            closed: VecDeque::new(),
            next_id: 1,
        })
    }

    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        self.listener.local_addr()
    }

    fn admit(&mut self, ws: WebSocket<TcpStream>) {
        let id = self.next_id;
        self.next_id += 1;
        self.clients.insert(id, ws);
    }

    fn pump(&mut self) {
        while self.pending.len() < MAX_PENDING_HANDSHAKES {
            match self.listener.accept() {
                Ok((stream, _)) => {
                    if stream.set_nonblocking(true).is_err() {
                        continue;
                    }
                    let _ = stream.set_nodelay(true);
                    match tungstenite::accept(stream) {
                        Ok(ws) => self.admit(ws),
                        Err(HandshakeError::Interrupted(mid)) => self.pending.push(mid),
                        Err(HandshakeError::Failure(_)) => {}
                    }
                }
                Err(_) => break,
            }
        }

        for mid in std::mem::take(&mut self.pending) {
            match mid.handshake() {
                Ok(ws) => self.admit(ws),
                Err(HandshakeError::Interrupted(mid)) => self.pending.push(mid),
                Err(HandshakeError::Failure(_)) => {}
            }
        }

        let mut dead = Vec::new();
        for (&id, ws) in self.clients.iter_mut() {
            loop {
                match ws.read() {
                    Ok(Message::Binary(b)) => {
                        if b.len() <= WS_MTU && self.inbox.len() < MAX_INBOX {
                            self.inbox.push_back((id, b.to_vec()));
                        }
                    }
                    Ok(Message::Close(_)) => {
                        dead.push(id);
                        break;
                    }
                    Ok(_) => {}
                    Err(tungstenite::Error::Io(e)) if e.kind() == io::ErrorKind::WouldBlock => {
                        break;
                    }
                    Err(_) => {
                        dead.push(id);
                        break;
                    }
                }
            }
            match ws.flush() {
                Ok(()) => {}
                Err(tungstenite::Error::Io(e)) if e.kind() == io::ErrorKind::WouldBlock => {}
                Err(_) => dead.push(id),
            }
        }
        for id in dead {
            if self.clients.remove(&id).is_some() {
                self.closed.push_back(id);
            }
        }
    }
}

impl Transport for WsServer {
    type Addr = u64;

    fn send(&mut self, to: u64, data: &[u8]) -> io::Result<()> {
        let ws = self
            .clients
            .get_mut(&to)
            .ok_or_else(|| io::Error::from(io::ErrorKind::NotConnected))?;
        match ws.send(Message::Binary(data.to_vec().into())) {
            Ok(()) => Ok(()),
            Err(tungstenite::Error::Io(e)) if e.kind() == io::ErrorKind::WouldBlock => Ok(()),
            Err(_) => {
                self.clients.remove(&to);
                self.closed.push_back(to);
                Err(io::Error::from(io::ErrorKind::BrokenPipe))
            }
        }
    }

    fn recv(&mut self, buf: &mut [u8]) -> io::Result<Option<(u64, usize)>> {
        if self.inbox.is_empty() {
            self.pump();
        }
        Ok(self.inbox.pop_front().map(|(id, data)| {
            let n = data.len().min(buf.len());
            buf[..n].copy_from_slice(&data[..n]);
            (id, n)
        }))
    }

    fn max_mtu(&self) -> usize {
        WS_MTU
    }

    fn is_reliable(&self, _peer: u64) -> bool {
        true
    }

    fn closed(&mut self) -> Option<u64> {
        self.closed.pop_front()
    }
}
