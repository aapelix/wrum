pub mod store;

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

pub const MAX_PACKET_SIZE: usize = 64 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Channel {
    Unreliable,
    Reliable,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConnState {
    Connecting,
    Open,
    Closed,
}

pub trait Transport {
    fn send(&mut self, ch: Channel, bytes: &[u8]) -> anyhow::Result<()>;
    fn poll(&mut self) -> Option<Vec<u8>>;
    fn state(&self) -> ConnState;
    fn error(&self) -> Option<String>;
}
