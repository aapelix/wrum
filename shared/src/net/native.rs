use std::{
    io,
    net::{SocketAddr, ToSocketAddrs, UdpSocket},
    time::Duration,
};

use crate::net::Transport;

pub struct UdpTransport {
    sock: UdpSocket,
}

impl UdpTransport {
    pub fn bind(addr: impl ToSocketAddrs) -> io::Result<Self> {
        let sock = UdpSocket::bind(addr)?;
        sock.set_nonblocking(true)?;
        Ok(Self { sock })
    }

    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        self.sock.local_addr()
    }
}

impl Transport for UdpTransport {
    type Addr = SocketAddr;

    fn send(&mut self, to: Self::Addr, data: &[u8]) -> io::Result<()> {
        self.sock.send_to(data, to).map(|_| ())
    }
    fn recv(&mut self, buf: &mut [u8]) -> io::Result<Option<(Self::Addr, usize)>> {
        loop {
            match self.sock.recv_from(buf) {
                Ok((n, from)) => return Ok(Some((from, n))),
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => return Ok(None),
                Err(e) if e.kind() == io::ErrorKind::ConnectionReset => continue,
                Err(e) => return Err(e),
            }
        }
    }

    fn wait(&mut self, timeout: std::time::Duration) {
        let timeout = timeout.max(Duration::from_millis(1));
        if self.sock.set_nonblocking(false).is_err() {
            return;
        }
        let _ = self.sock.set_read_timeout(Some(timeout));
        let _ = self.sock.peek_from(&mut [0u8; 1]);
        let _ = self.sock.set_nonblocking(true);
    }
}
