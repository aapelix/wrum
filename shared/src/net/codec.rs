use rkyv::{
    Archive, Deserialize,
    api::high::{HighDeserializer, HighSerializer, HighValidator, to_bytes_in},
    bytecheck::CheckBytes,
    rancor::Error,
    ser::allocator::ArenaHandle,
    util::AlignedVec,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CodecError;

impl std::fmt::Display for CodecError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("message encode/decode failed")
    }
}
impl std::error::Error for CodecError {}

#[derive(Default)]
pub struct Scratch(AlignedVec);

pub trait Message: Sized {
    fn encode<'a>(&self, scratch: &'a mut Scratch) -> Result<&'a [u8], CodecError>;
    fn decode(bytes: &[u8]) -> Result<Self, CodecError>;
}

impl<T> Message for T
where
    T: Archive + for<'a> rkyv::Serialize<HighSerializer<AlignedVec, ArenaHandle<'a>, Error>>,
    T::Archived:
        for<'a> CheckBytes<HighValidator<'a, Error>> + Deserialize<T, HighDeserializer<Error>>,
{
    fn encode<'a>(&self, scratch: &'a mut Scratch) -> Result<&'a [u8], CodecError> {
        let mut buf = std::mem::take(&mut scratch.0);
        buf.clear();
        scratch.0 = to_bytes_in::<_, Error>(self, buf).map_err(|_| CodecError)?;
        Ok(&scratch.0)
    }
    fn decode(bytes: &[u8]) -> Result<Self, CodecError> {
        rkyv::from_bytes::<T, Error>(bytes).map_err(|_| CodecError)
    }
}
