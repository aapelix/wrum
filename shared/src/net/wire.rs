pub const KIND_MASK: u8 = 0b111;
pub const KIND_DATA: u8 = 0;
pub const KIND_CONNECT: u8 = 1;
pub const KIND_CHALLENGE: u8 = 2;
pub const KIND_RESPONSE: u8 = 3;
pub const KIND_ACCEPT: u8 = 4;
pub const KIND_DISCONNECT: u8 = 5;

pub const HAS_ACK: u8 = 1 << 3;
pub const HAS_SEQ: u8 = 1 << 4;

pub const CH_MASK: u8 = 0b11;
pub const FRAG: u8 = 1 << 2;
pub const LAST: u8 = 1 << 3;

pub const MIN_CONNECT: usize = 64;

#[inline]
pub fn varint_len(v: u32) -> usize {
    match v {
        0..=0x7f => 1,
        0x80..=0x3fff => 2,
        0x4000..=0x1f_ffff => 3,
        0x20_0000..=0x0fff_ffff => 4,
        _ => 5,
    }
}
#[inline]
pub fn put_varint(buf: &mut [u8], pos: &mut usize, mut v: u32) {
    while v >= 0x80 {
        buf[*pos] = (v as u8) | 0x80;
        *pos += 1;
        v >>= 7;
    }
    buf[*pos] = v as u8;
    *pos += 1;
}

pub struct Reader<'a> {
    b: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    #[inline]
    pub fn new(b: &'a [u8]) -> Self {
        Self { b, pos: 0 }
    }
    #[inline]
    pub fn remaining(&self) -> usize {
        self.b.len() - self.pos
    }
    #[inline]
    pub fn u8(&mut self) -> Option<u8> {
        let v = *self.b.get(self.pos)?;
        self.pos += 1;
        Some(v)
    }
    #[inline]
    pub fn u16(&mut self) -> Option<u16> {
        Some(u16::from_le_bytes(self.take(2)?.try_into().ok()?))
    }
    #[inline]
    pub fn u32(&mut self) -> Option<u32> {
        Some(u32::from_le_bytes(self.take(4)?.try_into().ok()?))
    }
    #[inline]
    pub fn u64(&mut self) -> Option<u64> {
        Some(u64::from_le_bytes(self.take(8)?.try_into().ok()?))
    }
    pub fn varint(&mut self) -> Option<u32> {
        let mut v = 0u32;
        for shift in (0..35).step_by(7) {
            let b = self.u8()?;
            v |= ((b & 0x7f) as u32).checked_shl(shift)?;
            if b & 0x80 == 0 {
                return Some(v);
            }
        }
        None
    }
    #[inline]
    pub fn take(&mut self, n: usize) -> Option<&'a [u8]> {
        let end = self.pos.checked_add(n)?;
        let s = self.b.get(self.pos..end)?;
        self.pos = end;
        Some(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn varint_roundtrip() {
        for v in [0, 1, 127, 128, 16383, 16384, 2_097_151, 2_097_152, u32::MAX] {
            let mut b = [0u8; 8];
            let mut p = 0;
            put_varint(&mut b, &mut p, v);
            assert_eq!(p, varint_len(v));
            assert_eq!(Reader::new(&b[..p]).varint(), Some(v));
        }
    }

    #[test]
    fn varint_rejects_overlong() {
        assert_eq!(Reader::new(&[0xff; 8]).varint(), None);
    }
}
