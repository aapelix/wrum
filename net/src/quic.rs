use std::{
    net::SocketAddr,
    sync::{
        Arc, OnceLock,
        mpsc::{Receiver, Sender, TryRecvError},
    },
    time::Duration,
};

use anyhow::{Context, bail};
use quinn::{ClientConfig, Connection, Endpoint};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};

use crate::{Channel, Delivery, Transport};

const MAX_MESSAGE: usize = 1 << 20;

enum Event {
    Message(Channel, Vec<u8>),
    Closed(String),
}

pub struct QuicTransport {
    rt: tokio::runtime::Runtime,
    channels: Vec<Delivery>,
    out: Vec<UnboundedSender<Vec<u8>>>,
    seq: Vec<u32>,
    rx: Receiver<Event>,
    handle: Arc<OnceLock<(Endpoint, Connection)>>,
}

impl QuicTransport {
    pub fn open(
        addr: SocketAddr,
        server_name: &str,
        client_config: ClientConfig,
        channels: &[Delivery],
    ) -> anyhow::Result<Self> {
        if channels.len() > 256 {
            bail!("at most 256 channels");
        }
        let rt = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .enable_all()
            .build()?;

        let (in_tx, rx) = std::sync::mpsc::channel();
        let (out, receivers): (
            Vec<UnboundedSender<Vec<u8>>>,
            Vec<UnboundedReceiver<Vec<u8>>>,
        ) = channels.iter().map(|_| unbounded_channel()).unzip();
        let handle = Arc::new(OnceLock::new());

        rt.spawn(run(
            addr,
            server_name.to_owned(),
            client_config,
            channels.to_vec(),
            receivers,
            in_tx,
            Arc::clone(&handle),
        ));

        Ok(Self {
            rt,
            channels: channels.to_vec(),
            out,
            seq: vec![0; channels.len()],
            rx,
            handle,
        })
    }
}

pub fn client_config_pinned(cert_der: Vec<u8>) -> anyhow::Result<ClientConfig> {
    let mut roots = rustls::RootCertStore::empty();
    roots.add(rustls::pki_types::CertificateDer::from(cert_der))?;
    let mut cfg = ClientConfig::with_root_certificates(Arc::new(roots))?;

    let mut t = quinn::TransportConfig::default();
    t.keep_alive_interval(Some(Duration::from_secs(2)));
    t.max_idle_timeout(Some(Duration::from_secs(10).try_into()?));
    // t.congestion_controller_factory(Arc::new(quinn::congestion::BbrConfig::default()));
    cfg.transport_config(Arc::new(t));
    Ok(cfg)
}

impl Transport for QuicTransport {
    fn send(&mut self, ch: Channel, bytes: &[u8]) -> anyhow::Result<()> {
        let i = ch.0 as usize;
        let delivery = *self.channels.get(i).context("unknown channel")?;

        let frame = match delivery {
            Delivery::ReliableOrdered => {
                let mut v = Vec::with_capacity(4 + bytes.len());
                v.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
                v.extend_from_slice(bytes);
                v
            }
            Delivery::Unreliable => {
                let mut v = Vec::with_capacity(1 + bytes.len());
                v.push(ch.0);
                v.extend_from_slice(bytes);
                v
            }
            Delivery::UnreliableSequenced => {
                let s = self.seq[i];
                self.seq[i] = s.wrapping_add(1);
                let mut v = Vec::with_capacity(5 + bytes.len());
                v.push(ch.0);
                v.extend_from_slice(&s.to_le_bytes());
                v.extend_from_slice(bytes);
                v
            }
        };
        self.out[i]
            .send(frame)
            .map_err(|_| anyhow::anyhow!("connection closed"))
    }
    fn poll(&mut self) -> anyhow::Result<Option<(Channel, Vec<u8>)>> {
        match self.rx.try_recv() {
            Ok(Event::Message(ch, bytes)) => Ok(Some((ch, bytes))),
            Ok(Event::Closed(reason)) => bail!("connection closed: {reason}"),
            Err(TryRecvError::Empty) => Ok(None),
            Err(TryRecvError::Disconnected) => bail!("transport task stopped"),
        }
    }
}

impl Drop for QuicTransport {
    fn drop(&mut self) {
        if let Some((endpoint, conn)) = self.handle.get() {
            conn.close(0u32.into(), b"bye");
            let _ = self.rt.block_on(tokio::time::timeout(
                Duration::from_millis(200),
                endpoint.wait_idle(),
            ));
        }
    }
}

async fn run(
    addr: SocketAddr,
    server_name: String,
    cfg: ClientConfig,
    channels: Vec<Delivery>,
    receivers: Vec<UnboundedReceiver<Vec<u8>>>,
    in_tx: Sender<Event>,
    handle: Arc<OnceLock<(Endpoint, Connection)>>,
) {
    let result = drive(
        addr,
        server_name,
        cfg,
        channels,
        receivers,
        in_tx.clone(),
        handle,
    )
    .await;
    let reason = match result {
        Ok(()) => "closed".to_string(),
        Err(e) => format!("{e:#}"),
    };
    let _ = in_tx.send(Event::Closed(reason));
}
async fn drive(
    addr: SocketAddr,
    server_name: String,
    cfg: ClientConfig,
    channels: Vec<Delivery>,
    receivers: Vec<UnboundedReceiver<Vec<u8>>>,
    in_tx: Sender<Event>,
    handle: Arc<OnceLock<(Endpoint, Connection)>>,
) -> anyhow::Result<()> {
    let bind = if addr.is_ipv6() {
        "[::]:0"
    } else {
        "0.0.0.0:0"
    };
    let bind: SocketAddr = bind.parse()?;
    let mut endpoint = Endpoint::client(bind)?;
    endpoint.set_default_client_config(cfg);

    let conn = endpoint.connect(addr, &server_name)?.await?;
    let _ = handle.set((endpoint, conn.clone()));

    spawn_senders(&conn, &channels, receivers);
    tokio::spawn(accept_streams(
        conn.clone(),
        channels.clone(),
        in_tx.clone(),
    ));

    read_datagrams(&conn, &channels, &in_tx).await
}
fn spawn_senders(
    conn: &Connection,
    channels: &[Delivery],
    receivers: Vec<UnboundedReceiver<Vec<u8>>>,
) {
    for (i, (delivery, mut rx)) in channels.iter().copied().zip(receivers).enumerate() {
        let conn = conn.clone();
        tokio::spawn(async move {
            match delivery {
                Delivery::ReliableOrdered => {
                    let Ok(mut stream) = conn.open_uni().await else {
                        return;
                    };
                    if stream.write_all(&[i as u8]).await.is_err() {
                        return;
                    }
                    while let Some(frame) = rx.recv().await {
                        if stream.write_all(&frame).await.is_err() {
                            return;
                        }
                    }
                }
                Delivery::Unreliable | Delivery::UnreliableSequenced => {
                    while let Some(frame) = rx.recv().await {
                        let _ = conn.send_datagram(frame.into());
                    }
                }
            }
        });
    }
}
async fn read_datagrams(
    conn: &Connection,
    channels: &[Delivery],
    tx: &Sender<Event>,
) -> anyhow::Result<()> {
    let mut last_seq: Vec<Option<u32>> = vec![None; channels.len()];
    loop {
        let d = conn.read_datagram().await?;
        let Some((&ch, rest)) = d.split_first() else {
            continue;
        };
        let payload = match channels.get(ch as usize) {
            Some(Delivery::Unreliable) => rest,
            Some(Delivery::UnreliableSequenced) => {
                if rest.len() < 4 {
                    continue;
                }
                let seq = u32::from_le_bytes(rest[..4].try_into().unwrap());
                if last_seq[ch as usize].is_some_and(|last| seq <= last) {
                    continue;
                }
                last_seq[ch as usize] = Some(seq);
                &rest[4..]
            }
            _ => continue,
        };
        if tx
            .send(Event::Message(Channel(ch), payload.to_vec()))
            .is_err()
        {
            return Ok(());
        }
    }
}
async fn accept_streams(conn: Connection, channels: Vec<Delivery>, tx: Sender<Event>) {
    while let Ok(stream) = conn.accept_uni().await {
        let channels = channels.clone();
        let tx = tx.clone();
        tokio::spawn(async move {
            let _ = read_stream(stream, &channels, &tx).await;
        });
    }
}
async fn read_stream(
    mut stream: quinn::RecvStream,
    channels: &[Delivery],
    tx: &Sender<Event>,
) -> anyhow::Result<()> {
    let mut id = [0u8; 1];
    stream.read_exact(&mut id).await?;
    let ch = id[0];
    if channels.get(ch as usize) != Some(&Delivery::ReliableOrdered) {
        bail!("stream for non-reliable channel {ch}");
    }
    loop {
        let mut len = [0u8; 4];
        stream.read_exact(&mut len).await?;
        let len = u32::from_le_bytes(len) as usize;
        if len > MAX_MESSAGE {
            bail!("frame too large");
        }
        let mut buf = vec![0u8; len];
        stream.read_exact(&mut buf).await?;
        if tx.send(Event::Message(Channel(ch), buf)).is_err() {
            return Ok(());
        }
    }
}
