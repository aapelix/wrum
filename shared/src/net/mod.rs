use std::{fmt::Debug, hash::Hash, io, time::Duration};

pub mod codec;
pub mod conn;
pub mod dual;
pub mod endpoint;
pub mod proto;
pub mod window;
pub mod wire;

#[cfg(not(target_os = "emscripten"))]
pub mod native;
#[cfg(target_os = "emscripten")]
mod ws_emscripten;
#[cfg(target_os = "emscripten")]
pub mod web {
    pub use super::ws_emscripten::WsClient;
}

#[cfg(feature = "ws-server")]
pub mod ws_server;

pub use codec::{CodecError, Message, Scratch};
pub use conn::{Channel, Connection, Params, SendError, WINDOW};
pub use endpoint::{Config, ConnId, DisconnectReason, Endpoint, Event};
pub use proto::{ClientMessage, ServerMessage};
pub use rkyv;

#[cfg(feature = "ws-server")]
pub use ws_server::{WS_MTU, WsServer};

pub trait Transport {
    type Addr: Copy + Eq + Hash + Debug;

    fn send(&mut self, to: Self::Addr, data: &[u8]) -> io::Result<()>;
    fn recv(&mut self, buf: &mut [u8]) -> io::Result<Option<(Self::Addr, usize)>>;

    fn max_mtu(&self) -> usize {
        1200
    }
    fn mtu(&self, _peer: Self::Addr) -> usize {
        self.max_mtu()
    }

    fn is_reliable(&self, _peer: Self::Addr) -> bool {
        false
    }

    fn closed(&mut self) -> Option<Self::Addr> {
        None
    }
    fn wait(&mut self, timeout: Duration) {
        std::thread::sleep(timeout.min(Duration::from_millis(1)));
    }
}
