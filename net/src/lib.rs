pub mod proto;
#[cfg(target_os = "emscripten")]
pub mod ws;

pub trait Transport {
    fn send(&mut self, bytes: &[u8]) -> anyhow::Result<()>;
    fn poll(&mut self) -> anyhow::Result<Option<Vec<u8>>>;
}
