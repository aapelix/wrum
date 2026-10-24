use std::{
    ffi::CString,
    os::raw::{c_char, c_int},
};

use crate::{ConnState, MAX_PACKET_SIZE, Transport};

unsafe extern "C" {
    fn ws_open(url: *const c_char);
    fn ws_state() -> c_int;
    fn ws_send(ptr: *const u8, len: c_int) -> c_int;
    fn ws_poll(buf: *mut u8, cap: c_int) -> c_int;
    fn ws_close();
}

pub struct EmscriptenTransport {
    buf: Vec<u8>,
}

impl EmscriptenTransport {
    pub fn connect(url: &str) -> anyhow::Result<Self> {
        let c = CString::new(url)?;
        unsafe { ws_open(c.as_ptr()) };
        Ok(Self {
            buf: vec![0; MAX_PACKET_SIZE],
        })
    }
}

impl Transport for EmscriptenTransport {
    fn send(&mut self, bytes: &[u8]) -> anyhow::Result<()> {
        anyhow::ensure!(unsafe { ws_state() } == 1, "not connected");
        anyhow::ensure!(
            unsafe { ws_send(bytes.as_ptr(), bytes.len() as c_int) } == 0,
            "send failed"
        );
        Ok(())
    }
    fn poll(&mut self) -> Option<Vec<u8>> {
        loop {
            let n = unsafe { ws_poll(self.buf.as_mut_ptr(), self.buf.len() as c_int) };
            if n == 0 {
                return None;
            }
            if n > 0 {
                return Some(self.buf[..n as usize].to_vec());
            }
        }
    }
    fn state(&self) -> ConnState {
        match unsafe { ws_state() } {
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
        unsafe { ws_close() };
    }
}
