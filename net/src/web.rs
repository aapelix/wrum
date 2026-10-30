use std::{
    ffi::CString,
    os::raw::{c_char, c_int},
};

use crate::{Channel, ConnState, MAX_PACKET_SIZE, Transport};

unsafe extern "C" {
    fn net_open(ws: *const c_char, wt: *const c_char, hash: *const c_char);
    fn net_state() -> c_int;
    fn net_kind() -> c_int;
    fn net_send(ch: c_int, ptr: *const u8, len: c_int) -> c_int;
    fn net_poll(buf: *mut u8, cap: c_int) -> c_int;
    fn net_close();
}

pub struct EmscriptenTransport {
    buf: Vec<u8>,
}

impl EmscriptenTransport {
    pub fn connect(_url: &str) -> anyhow::Result<Self> {
        let (a, b, c) = (
            CString::new("/ws")?,
            CString::new(":4433")?,
            CString::new("")?,
        );
        unsafe { net_open(a.as_ptr(), b.as_ptr(), c.as_ptr()) };
        Ok(Self {
            buf: vec![0; MAX_PACKET_SIZE],
        })
    }
}

impl Transport for EmscriptenTransport {
    fn send(&mut self, ch: Channel, bytes: &[u8]) -> anyhow::Result<()> {
        let c = if ch == Channel::Unreliable { 1 } else { 0 };
        anyhow::ensure!(
            unsafe { net_send(c, bytes.as_ptr(), bytes.len() as c_int) } == 0,
            "send failed"
        );
        Ok(())
    }
    fn poll(&mut self) -> Option<Vec<u8>> {
        loop {
            let n = unsafe { net_poll(self.buf.as_mut_ptr(), self.buf.len() as c_int) };
            if n == 0 {
                return None;
            }
            if n > 0 {
                return Some(self.buf[..n as usize].to_vec());
            }
        }
    }
    fn state(&self) -> ConnState {
        match unsafe { net_state() } {
            0 => ConnState::Connecting,
            1 => ConnState::Open,
            _ => ConnState::Closed,
        }
    }
    fn error(&self) -> Option<String> {
        None
    }
}

impl Drop for EmscriptenTransport {
    fn drop(&mut self) {
        unsafe { net_close() };
    }
}
