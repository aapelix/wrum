use std::sync::{
    Arc, Mutex, OnceLock,
    atomic::{AtomicU8, Ordering},
};

use futures_channel::mpsc::{UnboundedReceiver, UnboundedSender, unbounded};
use futures_util::{SinkExt, StreamExt};
use tokio_tungstenite::tungstenite::Message;

use crate::{ConnState, Transport};

const CONNECTING: u8 = 0;
const OPEN: u8 = 1;
const CLOSED: u8 = 2;

struct Shared {
    state: AtomicU8,
    error: Mutex<Option<String>>,
}

pub struct NativeTransport {
    shared: Arc<Shared>,
    out: UnboundedSender<Vec<u8>>,
    incoming: UnboundedReceiver<Vec<u8>>,
}

fn runtime() -> &'static tokio::runtime::Runtime {
    static RT: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
    RT.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .enable_all()
            .build()
            .unwrap()
    })
}

impl NativeTransport {
    pub fn connect(url: &str) -> anyhow::Result<Self> {
        let shared = Arc::new(Shared {
            state: AtomicU8::new(CONNECTING),
            error: Mutex::new(None),
        });
        let (out, out_rx) = unbounded::<Vec<u8>>();
        let (in_tx, incoming) = unbounded::<Vec<u8>>();
        let (url, sh) = (url.to_string(), shared.clone());
        runtime().spawn(async move {
            if let Err(e) = run(&url, &sh, out_rx, in_tx).await {
                *sh.error.lock().unwrap() = Some(e.to_string());
            }
            sh.state.store(CLOSED, Ordering::Release);
        });
        Ok(Self {
            shared,
            out,
            incoming,
        })
    }
}

async fn run(
    url: &str,
    shared: &Shared,
    mut out_rx: UnboundedReceiver<Vec<u8>>,
    in_tx: UnboundedSender<Vec<u8>>,
) -> anyhow::Result<()> {
    let (ws, _) = tokio_tungstenite::connect_async_with_config(url, None, true).await?;
    shared.state.store(OPEN, Ordering::Release);
    let (mut sink, mut stream) = ws.split();
    let writer = tokio::spawn(async move {
        while let Some(b) = out_rx.next().await {
            if sink.send(Message::Binary(b.into())).await.is_err() {
                break;
            }
        }
    });
    while let Some(m) = stream.next().await {
        match m? {
            Message::Binary(b) => {
                if in_tx.unbounded_send(b.to_vec()).is_err() {
                    break;
                }
            }
            Message::Close(_) => break,
            _ => {}
        }
    }
    writer.abort();
    Ok(())
}

impl Transport for NativeTransport {
    fn send(&mut self, bytes: &[u8]) -> anyhow::Result<()> {
        anyhow::ensure!(
            self.shared.state.load(Ordering::Acquire) == OPEN,
            "not connected"
        );
        self.out
            .unbounded_send(bytes.to_vec())
            .map_err(|e| anyhow::anyhow!("{e}"))
    }
    fn poll(&mut self) -> Option<Vec<u8>> {
        self.incoming.try_recv().ok()
    }
    fn state(&self) -> ConnState {
        match self.shared.state.load(Ordering::Acquire) {
            CONNECTING => ConnState::Connecting,
            OPEN => ConnState::Open,
            _ => ConnState::Closed,
        }
    }
    fn error(&self) -> Option<String> {
        self.shared.error.lock().unwrap().clone()
    }
}
