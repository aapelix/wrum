use std::sync::{
    Arc, Mutex, OnceLock,
    atomic::{AtomicU8, Ordering},
};

use futures_channel::mpsc::{TryRecvError, UnboundedReceiver, UnboundedSender, unbounded};
use futures_util::StreamExt;
use wtransport::{ClientConfig, Connection, Endpoint, RecvStream, VarInt};

use crate::{Channel, MAX_PACKET_SIZE, Packet, Transport};

const CONNECTING: u8 = 0;
const OPEN: u8 = 1;
const CLOSED: u8 = 2;

struct Shared {
    state: AtomicU8,
    error: Mutex<Option<String>>,
    conn: OnceLock<Connection>,
}
impl Shared {
    fn close(&self, reason: Option<String>) {
        let mut e = self.error.lock().unwrap();
        if e.is_none() {
            *e = reason;
        }
        self.state.store(CLOSED, Ordering::Release);
    }
}

pub struct NativeTransport {
    shared: Arc<Shared>,
    reliable_tx: UnboundedSender<Vec<u8>>,
    incoming: UnboundedReceiver<Packet>,
}

fn runtime() -> &'static tokio::runtime::Runtime {
    static RT: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
    RT.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .enable_all()
            .build()
            .expect("tokio runtime")
    })
}

impl NativeTransport {
    pub fn connect(url: &str) -> anyhow::Result<Self> {
        let shared = Arc::new(Shared {
            state: AtomicU8::new(CONNECTING),
            error: Mutex::new(None),
            conn: OnceLock::new(),
        });
        let (reliable_tx, reliable_rx) = unbounded::<Vec<u8>>();
        let (incoming_tx, incoming) = unbounded::<Packet>();

        let (url, sh) = (url.to_string(), shared.clone());
        runtime().spawn(async move {
            if let Err(e) = run(url, sh.clone(), reliable_rx, incoming_tx).await {
                sh.close(Some(e.to_string()));
            }
        });
        Ok(Self {
            shared,
            reliable_tx,
            incoming,
        })
    }
}

async fn run(
    url: String,
    shared: Arc<Shared>,
    mut reliable_rx: UnboundedReceiver<Vec<u8>>,
    incoming_tx: UnboundedSender<Packet>,
) -> anyhow::Result<()> {
    let config = ClientConfig::default();
    let conn = Endpoint::client(config)?.connect(&url).await?;

    if shared.state.load(Ordering::Acquire) == CLOSED {
        conn.close(VarInt::from_u32(0), b"cancelled");
        return Ok(());
    }

    let mut send = conn.open_uni().await?.await?;
    let _ = shared.conn.set(conn.clone());

    // reliable send
    {
        let shared = shared.clone();
        tokio::spawn(async move {
            while let Some(bytes) = reliable_rx.next().await {
                let mut framed = Vec::with_capacity(4 + bytes.len());
                framed.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
                framed.extend_from_slice(&bytes);
                if send.write_all(&framed).await.is_err() {
                    shared.close(None);
                    break;
                }
            }
        });
    }

    // unreliable recv
    {
        let conn = conn.clone();
        let tx = incoming_tx.clone();
        let shared = shared.clone();
        tokio::spawn(async move {
            loop {
                match conn.receive_datagram().await {
                    Ok(dg) => {
                        if tx
                            .unbounded_send((Channel::Unreliable, dg.payload().to_vec()))
                            .is_err()
                        {
                            break;
                        }
                    }
                    Err(_) => {
                        shared.close(None);
                        break;
                    }
                }
            }
        });
    }

    // reliable recv
    {
        let conn = conn.clone();
        let shared = shared.clone();
        tokio::spawn(async move {
            loop {
                match conn.accept_uni().await {
                    Ok(stream) => {
                        let tx = incoming_tx.clone();
                        let shared = shared.clone();
                        tokio::spawn(async move {
                            if let Err(e) = read_reliable_stream(stream, tx).await {
                                eprintln!("reliable stream error: {e}");
                                shared.close(None);
                            }
                        });
                    }
                    Err(_) => {
                        shared.close(None);
                        break;
                    }
                }
            }
        });
    }

    // closed watcher
    {
        let conn = conn.clone();
        let shared = shared.clone();
        tokio::spawn(async move {
            let _ = conn.closed().await;
            shared.close(Some(conn.closed().await.to_string()))
        });
    }

    shared.state.store(OPEN, Ordering::Release);
    Ok(())
}

impl Transport for NativeTransport {
    fn send(&mut self, ch: Channel, bytes: &[u8]) -> anyhow::Result<()> {
        anyhow::ensure!(
            self.shared.state.load(Ordering::Acquire) == OPEN,
            "not connected"
        );
        match ch {
            Channel::Reliable => self
                .reliable_tx
                .unbounded_send(bytes.to_vec())
                .map_err(|e| anyhow::anyhow!("send queue closed: {e}")),
            Channel::Unreliable => {
                let conn = self
                    .shared
                    .conn
                    .get()
                    .ok_or_else(|| anyhow::anyhow!("not connected"))?;
                let max = conn.max_datagram_size().unwrap_or(0);
                anyhow::ensure!(
                    bytes.len() <= max,
                    "datagram too large: {} > {max}",
                    bytes.len()
                );
                conn.send_datagram(bytes)?;
                Ok(())
            }
        }
    }
    fn poll(&mut self) -> anyhow::Result<Option<(Channel, Vec<u8>)>> {
        match self.incoming.try_recv() {
            Ok(p) => Ok(Some(p)),
            Err(TryRecvError::Empty) => Ok(None),
            Err(e) => Err(anyhow::anyhow!("recv queue error: {e}")),
        }
    }
    fn state(&self) -> crate::ConnState {
        match self.shared.state.load(Ordering::Acquire) {
            CONNECTING => crate::ConnState::Connecting,
            OPEN => crate::ConnState::Open,
            _ => crate::ConnState::Closed,
        }
    }
    fn error(&self) -> Option<String> {
        self.shared.error.lock().unwrap().clone()
    }
}

impl Drop for NativeTransport {
    fn drop(&mut self) {
        self.shared.close(None);
        if let Some(c) = self.shared.conn.get() {
            c.close(VarInt::from_u32(0), b"client closed");
        }
    }
}

async fn read_reliable_stream(
    mut stream: RecvStream,
    tx: UnboundedSender<Packet>,
) -> anyhow::Result<()> {
    loop {
        let mut hdr = [0u8; 4];
        match stream.read_exact(&mut hdr).await {
            Ok(_) => {}
            Err(wtransport::error::StreamReadExactError::FinishedEarly(_)) => return Ok(()),
            Err(e) => return Err(e.into()),
        }
        let len = u32::from_le_bytes(hdr) as usize;
        anyhow::ensure!(len <= MAX_PACKET_SIZE, "reliable packet too large");

        let mut buf = vec![0u8; len];
        stream.read_exact(&mut buf).await?;
        tx.unbounded_send((Channel::Reliable, buf))
            .map_err(|_| anyhow::anyhow!("recv queue closed"))?;
    }
}
