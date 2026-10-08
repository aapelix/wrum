//! Host two transports behind one `Endpoint`, e.g. native clients over UDP and
//! browser clients over WebSocket. Peers keep their own reliability/MTU.

use std::{io, time::Duration};

use crate::net::Transport;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Either<A, B> {
    A(A),
    B(B),
}

pub struct Dual<A, B> {
    pub a: A,
    pub b: B,
    flip: bool,
}

impl<A, B> Dual<A, B> {
    pub fn new(a: A, b: B) -> Self {
        Self { a, b, flip: false }
    }
}

impl<A: Transport, B: Transport> Transport for Dual<A, B> {
    type Addr = Either<A::Addr, B::Addr>;

    fn send(&mut self, to: Self::Addr, data: &[u8]) -> io::Result<()> {
        match to {
            Either::A(x) => self.a.send(x, data),
            Either::B(x) => self.b.send(x, data),
        }
    }

    fn recv(&mut self, buf: &mut [u8]) -> io::Result<Option<(Self::Addr, usize)>> {
        self.flip = !self.flip;
        for first_a in [self.flip, !self.flip] {
            if first_a {
                if let Some((x, n)) = self.a.recv(buf)? {
                    return Ok(Some((Either::A(x), n)));
                }
            } else if let Some((x, n)) = self.b.recv(buf)? {
                return Ok(Some((Either::B(x), n)));
            }
        }
        Ok(None)
    }

    fn max_mtu(&self) -> usize {
        self.a.max_mtu().max(self.b.max_mtu())
    }

    fn mtu(&self, peer: Self::Addr) -> usize {
        match peer {
            Either::A(x) => self.a.mtu(x),
            Either::B(x) => self.b.mtu(x),
        }
    }

    fn is_reliable(&self, peer: Self::Addr) -> bool {
        match peer {
            Either::A(x) => self.a.is_reliable(x),
            Either::B(x) => self.b.is_reliable(x),
        }
    }

    fn closed(&mut self) -> Option<Self::Addr> {
        self.a
            .closed()
            .map(Either::A)
            .or_else(|| self.b.closed().map(Either::B))
    }

    fn wait(&mut self, timeout: Duration) {
        std::thread::sleep(timeout.min(Duration::from_millis(1)));
    }
}
