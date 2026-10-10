pub mod proto;

#[cfg(not(target_os = "emscripten"))]
pub mod native;
#[cfg(target_os = "emscripten")]
pub mod wasm;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Channel {
    Unreliable,
    Reliable,
}

pub type Packet = (Channel, Vec<u8>);

pub trait Transport {
    fn send(&mut self, ch: Channel, bytes: &[u8]) -> anyhow::Result<()>;
    fn poll(&mut self) -> anyhow::Result<Option<(Channel, Vec<u8>)>>;
    fn is_connected(&self) -> bool;
}
