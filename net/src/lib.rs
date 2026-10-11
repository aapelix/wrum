pub mod proto;

#[cfg(not(target_os = "emscripten"))]
mod native;
#[cfg(not(target_os = "emscripten"))]
pub use native::NativeTransport as PlatformTransport;
#[cfg(target_os = "emscripten")]
mod web;
#[cfg(target_os = "emscripten")]
pub use web::EmscriptenTransport as PlatformTransport;

pub fn connect(url: &str) -> anyhow::Result<PlatformTransport> {
    PlatformTransport::connect(url)
}

pub const MAX_PACKET_SIZE: usize = 1024 * 1024;

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
