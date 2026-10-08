use std::{
    ffi::{CString, c_char, c_int},
    io,
    time::Duration,
};

use crate::net::Transport;

unsafe extern "C" {
    fn tn_ws_open(url: *const c_char) -> c_int;
    fn tn_ws_state(id: c_int) -> c_int;
    fn tn_ws_send(id: c_int, ptr: *const u8, len: usize) -> c_int;
    fn tn_ws_recv(id: c_int, ptr: *mut u8, cap: usize) -> c_int;
    fn tn_ws_close(id: c_int);
}

const CONNECTING: c_int = 0;
const OPEN: c_int = 1;
const MAX_QUEUED: usize = 64;
const MTU: usize = 64 * 1024;

pub struct WsClient {
    id: c_int,
    queued: Vec<Vec<u8>>,
    reported_closed: bool,
}

impl WsClient {
    pub fn connect(url: &str) -> io::Result<Self> {
        let c = CString::new(url).map_err(|_| io::Error::from(io::ErrorKind::InvalidInput))?;
        let id = unsafe { tn_ws_open(c.as_ptr()) };
        Ok(Self {
            id,
            queued: Vec::new(),
            reported_closed: false,
        })
    }

    fn state(&self) -> c_int {
        unsafe { tn_ws_state(self.id) }
    }

    fn flush_queued(&mut self) {
        if self.queued.is_empty() || self.state() != OPEN {
            return;
        }
        for d in std::mem::take(&mut self.queued) {
            unsafe { tn_ws_send(self.id, d.as_ptr(), d.len()) };
        }
    }
}

impl Transport for WsClient {
    type Addr = ();

    fn send(&mut self, _to: (), data: &[u8]) -> io::Result<()> {
        match self.state() {
            OPEN => {
                self.flush_queued();
                match unsafe { tn_ws_send(self.id, data.as_ptr(), data.len()) } {
                    0 => Ok(()),
                    _ => Err(io::Error::from(io::ErrorKind::BrokenPipe)),
                }
            }
            CONNECTING => {
                if self.queued.len() < MAX_QUEUED {
                    self.queued.push(data.to_vec());
                }
                Ok(())
            }
            _ => Err(io::Error::from(io::ErrorKind::BrokenPipe)),
        }
    }

    fn recv(&mut self, buf: &mut [u8]) -> io::Result<Option<((), usize)>> {
        self.flush_queued();
        loop {
            let n = unsafe { tn_ws_recv(self.id, buf.as_mut_ptr(), buf.len()) };
            match n {
                n if n >= 0 => return Ok(Some(((), n as usize))),
                -2 => continue,
                _ => return Ok(None),
            }
        }
    }

    fn max_mtu(&self) -> usize {
        MTU
    }

    fn is_reliable(&self, _peer: ()) -> bool {
        true
    }

    fn closed(&mut self) -> Option<()> {
        if !self.reported_closed && self.state() > OPEN {
            self.reported_closed = true;
            return Some(());
        }
        None
    }

    fn wait(&mut self, _timeout: Duration) {}
}

impl Drop for WsClient {
    fn drop(&mut self) {
        unsafe { tn_ws_close(self.id) };
    }
}
