pub mod native;
pub mod proto;

pub trait Transport {
    fn send(&mut self, data: &[u8]);
    fn poll(&mut self, packets: &mut Vec<Vec<u8>>);
}
