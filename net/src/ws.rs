use std::{cell::RefCell, collections::VecDeque, rc::Rc};

use wasm_bindgen::{JsCast, prelude::Closure};
use web_sys::{MessageEvent, WebSocket};

use crate::Transport;

pub struct WsTransport {
    ws: WebSocket,
    incoming: Rc<RefCell<VecDeque<Vec<u8>>>>,
}

impl WsTransport {
    pub fn open(url: &str) -> anyhow::Result<Self> {
        let ws = WebSocket::new(url).map_err(js_error)?;
        ws.set_binary_type(web_sys::BinaryType::Arraybuffer);

        let incoming = Rc::new(RefCell::new(VecDeque::new()));
        let queue = Rc::clone(&incoming);

        let onmessage = Closure::wrap(Box::new(move |e: MessageEvent| {
            if let Ok(buf) = e.data().dyn_into::<js_sys::ArrayBuffer>() {
                let data = js_sys::Uint8Array::new(&buf).to_vec();
                queue.borrow_mut().push_back(data);
            }
        }) as Box<dyn FnMut(MessageEvent)>);
        ws.set_onmessage(Some(onmessage.as_ref().unchecked_ref()));

        Ok(Self { ws, incoming })
    }
}

impl Transport for WsTransport {
    fn send(&mut self, bytes: &[u8]) -> anyhow::Result<()> {
        self.ws.send_with_u8_array(bytes).map_err(js_error)
    }
    fn poll(&mut self) -> anyhow::Result<Option<Vec<u8>>> {
        Ok(self.incoming.borrow_mut().pop_front())
    }
}

fn js_error(e: wasm_bindgen::JsValue) -> anyhow::Error {
    anyhow::anyhow!("{e:?}")
}
