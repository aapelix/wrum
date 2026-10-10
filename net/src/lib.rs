pub mod proto;

#[cfg(not(target_os = "emscripten"))]
pub mod quic;
#[cfg(target_os = "emscripten")]
pub mod ws;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Delivery {
    Unreliable,
    UnreliableSequenced,
    ReliableOrdered,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Channel(pub u8);

pub trait Transport {
    fn send(&mut self, ch: Channel, bytes: &[u8]) -> anyhow::Result<()>;
    fn poll(&mut self) -> anyhow::Result<Option<(Channel, Vec<u8>)>>;
}
