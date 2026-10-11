use std::sync::{
    Arc, OnceLock,
    atomic::{AtomicBool, Ordering},
};

use futures_channel::mpsc::{TryRecvError, UnboundedReceiver, UnboundedSender, unbounded};
use futures_util::StreamExt;
use wtransport::{ClientConfig, Connection, Endpoint, RecvStream, VarInt};

use crate::{Channel, MAX_PACKET_SIZE, Packet, Transport};

static RT: OnceLock<tokio::runtime::Runtime> = OnceLock::new();

pub struct NativeTransport {
    conn: Connection,
    reliable_tx: UnboundedSender<Vec<u8>>,
    incoming: UnboundedReceiver<Packet>,
    connected: Arc<AtomicBool>,
}

impl NativeTransport {
    pub fn connect(url: &str) -> anyhow::Result<Self> {
        let rt = RT.get_or_init(|| {
            tokio::runtime::Builder::new_multi_thread()
                .worker_threads(1)
                .enable_all()
                .build()
                .expect("tokio runtime")
        });
        rt.block_on(Self::open(url))
    }
    pub async fn open(url: &str) -> anyhow::Result<Self> {
        let config = ClientConfig::default();
        let conn = Endpoint::client(config)?.connect(url).await?;

        let mut send = conn.open_uni().await?.await?;

        let (reliable_tx, mut reliable_rx) = unbounded::<Vec<u8>>();
        let (incoming_tx, incoming) = unbounded::<Packet>();
        let connected = Arc::new(AtomicBool::new(true));

        // reliable send
        {
            let connected = connected.clone();
            tokio::spawn(async move {
                while let Some(bytes) = reliable_rx.next().await {
                    let mut framed = Vec::with_capacity(4 + bytes.len());
                    framed.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
                    framed.extend_from_slice(&bytes);
                    if send.write_all(&framed).await.is_err() {
                        connected.store(false, Ordering::Relaxed);
                        break;
                    }
                }
            });
        }

        // unreliable recv
        {
            let conn = conn.clone();
            let tx = incoming_tx.clone();
            let connected = connected.clone();
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
                            connected.store(false, Ordering::Relaxed);
                            break;
                        }
                    }
                }
            });
        }

        // reliable recv
        {
            let conn = conn.clone();
            let connected = connected.clone();
            tokio::spawn(async move {
                loop {
                    match conn.accept_uni().await {
                        Ok(stream) => {
                            let tx = incoming_tx.clone();
                            let connected = connected.clone();
                            tokio::spawn(async move {
                                if let Err(e) = read_reliable_stream(stream, tx).await {
                                    eprintln!("reliable stream error: {e}");
                                    connected.store(false, Ordering::Relaxed);
                                }
                            });
                        }
                        Err(_) => {
                            connected.store(false, Ordering::Relaxed);
                            break;
                        }
                    }
                }
            });
        }

        // closed watcher
        {
            let conn = conn.clone();
            let connected = connected.clone();
            tokio::spawn(async move {
                let _ = conn.closed().await;
                connected.store(false, Ordering::Relaxed);
            });
        }

        Ok(Self {
            conn,
            reliable_tx,
            incoming,
            connected,
        })
    }
}

impl Transport for NativeTransport {
    fn send(&mut self, ch: Channel, bytes: &[u8]) -> anyhow::Result<()> {
        match ch {
            Channel::Reliable => self
                .reliable_tx
                .unbounded_send(bytes.to_vec())
                .map_err(|e| anyhow::anyhow!("send queue closed: {e}")),
            Channel::Unreliable => {
                let max = self.conn.max_datagram_size().unwrap_or(0);
                anyhow::ensure!(
                    bytes.len() <= max,
                    "datagram too large: {} > {max}",
                    bytes.len()
                );
                self.conn.send_datagram(bytes)?;
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
    fn is_connected(&self) -> bool {
        self.connected.load(Ordering::Relaxed)
    }
}

impl Drop for NativeTransport {
    fn drop(&mut self) {
        self.connected.store(false, Ordering::Relaxed);
        self.conn.close(VarInt::from_u32(0), b"client closed");
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
