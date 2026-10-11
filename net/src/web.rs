use std::{
    ffi::CString,
    os::raw::{c_char, c_int},
};

use crate::{Channel, MAX_PACKET_SIZE, Transport};

unsafe extern "C" {
    fn wt_open(url: *const c_char);
    fn wt_state() -> c_int;
    fn wt_send(ch: c_int, ptr: *const u8, len: c_int) -> c_int;
    fn wt_poll(buf: *mut u8, cap: c_int) -> c_int;
    fn wt_max_datagram_size() -> c_int;
    fn wt_close();
}

pub struct EmscriptenTransport {
    buf: Vec<u8>,
}

impl EmscriptenTransport {
    pub fn connect(url: &str) -> anyhow::Result<Self> {
        let c = CString::new(url)?;
        unsafe {
            wt_open(c.as_ptr());
        };
        Ok(Self {
            buf: vec![0; MAX_PACKET_SIZE + 1],
        })
    }
}

impl Transport for EmscriptenTransport {
    fn send(&mut self, ch: Channel, bytes: &[u8]) -> anyhow::Result<()> {
        anyhow::ensure!(unsafe { wt_state() } == 1, "not connected");
        if ch == Channel::Unreliable {
            let max = unsafe { wt_max_datagram_size() } as usize;
            anyhow::ensure!(
                bytes.len() <= max,
                "datagram too large: {} > {max}",
                bytes.len()
            );
        }
        let c = if ch == Channel::Reliable { 0 } else { 1 };
        let r = unsafe { wt_send(c, bytes.as_ptr(), bytes.len() as c_int) };
        anyhow::ensure!(r == 0, "send failed");
        Ok(())
    }
    fn poll(&mut self) -> anyhow::Result<Option<(Channel, Vec<u8>)>> {
        let n = unsafe { wt_poll(self.buf.as_mut_ptr(), self.buf.len() as c_int) };
        if n <= 0 {
            return Ok(None);
        }
        let ch = if self.buf[0] == 0 {
            Channel::Reliable
        } else {
            Channel::Unreliable
        };
        Ok(Some((ch, self.buf[1..n as usize].to_vec())))
    }
    fn is_connected(&self) -> bool {
        unsafe { wt_state() == 1 }
    }
}

impl Drop for EmscriptenTransport {
    fn drop(&mut self) {
        unsafe {
            wt_close();
        }
    }
}
