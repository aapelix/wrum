use std::{
    collections::HashMap,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};

use futures_channel::{
    mpsc::{UnboundedReceiver, UnboundedSender, unbounded},
    oneshot,
};
use futures_util::StreamExt;
use rand::RngExt;
use wrum_core::{
    PlayerId,
    physics::{PhysicsWorld, Vec2},
};
use wrum_net::{
    MAX_PACKET_SIZE,
    proto::{ClientMsg, RejectReason, ServerMsg, TICK_RATE, decode_client, encode_server},
};
use wtransport::{
    Connection, Endpoint, Identity, RecvStream, ServerConfig, endpoint::IncomingSession,
};

static NEXT_ID: AtomicU64 = AtomicU64::new(0);

#[derive(Clone)]
struct PlayerHandle {
    conn: Connection,
    reliable_tx: UnboundedSender<Vec<u8>>,
}

enum JoinError {
    Full,
    Gone,
}
enum LobbyCmd {
    Join {
        id: PlayerId,
        handle: PlayerHandle,
        reply: oneshot::Sender<Result<(), JoinError>>,
    },
    Leave {
        id: PlayerId,
    },
    Msg {
        id: PlayerId,
        msg: ClientMsg,
    },
}

type Lobbies = Arc<Mutex<HashMap<String, UnboundedSender<LobbyCmd>>>>;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let identity =
        Identity::load_pemfiles("certs/localhost.pem", "certs/localhost-key.pem").await?;
    let config = ServerConfig::builder()
        .with_bind_default(7778)
        .with_identity(identity)
        .keep_alive_interval(Some(Duration::from_secs(3)))
        .build();
    let server = Endpoint::server(config)?;
    let lobbies: Lobbies = Default::default();

    loop {
        let incoming = server.accept().await;
        let lobbies = lobbies.clone();
        tokio::spawn(async move {
            if let Err(e) = handle_session(incoming, lobbies).await {
                eprintln!("session: {e}");
            }
        });
    }
}

async fn handle_session(incoming: IncomingSession, lobbies: Lobbies) -> anyhow::Result<()> {
    let conn = incoming.await?.accept().await?;

    // server -> client reliable
    let mut send = conn.open_uni().await?.await?;
    let (reliable_tx, mut reliable_rx) = unbounded::<Vec<u8>>();
    tokio::spawn(async move {
        while let Some(b) = reliable_rx.next().await {
            let mut f = Vec::with_capacity(4 + b.len());
            f.extend_from_slice(&(b.len() as u32).to_le_bytes());
            f.extend_from_slice(&b);
            if send.write_all(&f).await.is_err() {
                break;
            }
        }
    });

    let (in_tx, mut in_rx) = unbounded::<Vec<u8>>();

    // client -> server unreliable
    {
        let (c, tx) = (conn.clone(), in_tx.clone());
        tokio::spawn(async move {
            while let Ok(dg) = c.receive_datagram().await {
                let _ = tx.unbounded_send(dg.payload().to_vec());
            }
        });
    }
    // client -> server reliable
    {
        let (c, tx) = (conn.clone(), in_tx.clone());
        tokio::spawn(async move {
            while let Ok(stream) = c.accept_uni().await {
                let tx = tx.clone();
                tokio::spawn(read_reliable_stream(stream, tx));
            }
        });
    }

    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    let me = PlayerHandle {
        conn: conn.clone(),
        reliable_tx,
    };
    let mut current: Option<(String, UnboundedSender<LobbyCmd>)> = None;

    println!("client joined: {id}");

    let closed = conn.closed();
    tokio::pin!(closed);

    loop {
        tokio::select! {
            _ = &mut closed => break,
            m = in_rx.next() => {
                let Some(bytes) = m else { break };
                let Ok(msg) = decode_client(&bytes) else { continue };

                match msg {
                    ClientMsg::CreateLobby => {
                        leave_current(&mut current, id);
                        let (lid, tx) = create_lobby(&lobbies, id);
                        join(&mut current, lid, tx, id, &me).await;
                    }
                    ClientMsg::JoinLobby { code } => {
                        leave_current(&mut current, id);
                        match find_lobby(&lobbies, &code) {
                            Some(tx) => join(&mut current, code, tx, id, &me).await,
                            None => send_reject(&me, RejectReason::LobbyNotFound),
                        }
                    }
                    ClientMsg::LeaveLobby => leave_current(&mut current, id),
                    msg => {
                        if let Some((_, tx)) = &current {
                            if tx.unbounded_send(LobbyCmd::Msg { id, msg }).is_err() {
                                current = None;
                                send_reject(&me, RejectReason::LobbyGone);
                            }
                        }
                    }
                }
            }
        }
    }
    leave_current(&mut current, id);
    println!("client left: {id}");
    Ok(())
}

fn leave_current(current: &mut Option<(String, UnboundedSender<LobbyCmd>)>, id: PlayerId) {
    if let Some((_, tx)) = current.take() {
        let _ = tx.unbounded_send(LobbyCmd::Leave { id });
    }
}
fn create_lobby(l: &Lobbies, host: PlayerId) -> (String, UnboundedSender<LobbyCmd>) {
    let lid = code();
    let (tx, rx) = unbounded();
    tokio::spawn(run_lobby(rx, host));
    l.lock().unwrap().insert(lid.clone(), tx.clone());
    (lid, tx)
}
async fn join(
    current: &mut Option<(String, UnboundedSender<LobbyCmd>)>,
    lobby_id: String,
    tx: UnboundedSender<LobbyCmd>,
    id: PlayerId,
    me: &PlayerHandle,
) {
    let (reply, rx) = oneshot::channel();
    if tx
        .unbounded_send(LobbyCmd::Join {
            id,
            handle: me.clone(),
            reply,
        })
        .is_err()
    {
        return send_reject(me, RejectReason::LobbyGone);
    }
    match rx.await {
        Ok(Ok(())) => *current = Some((lobby_id, tx)),
        Ok(Err(JoinError::Full)) => send_reject(me, RejectReason::LobbyFull),
        _ => send_reject(me, RejectReason::LobbyGone),
    }
}
fn find_lobby(l: &Lobbies, id: &str) -> Option<UnboundedSender<LobbyCmd>> {
    let mut map = l.lock().unwrap();
    match map.get(id) {
        Some(tx) if !tx.is_closed() => Some(tx.clone()),
        Some(_) => {
            map.remove(id);
            None
        }
        None => None,
    }
}

async fn run_lobby(mut rx: UnboundedReceiver<LobbyCmd>, _host: u64) {
    let mut players: HashMap<PlayerId, PlayerHandle> = HashMap::new();
    let mut physics = PhysicsWorld::default();

    let mut tick = tokio::time::interval(Duration::from_millis(TICK_RATE as u64));
    tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    let mut tick_no: u32 = 0;

    loop {
        tokio::select! {
            cmd = rx.next() => match cmd {
                None => break,
                Some(LobbyCmd::Join { id, handle, reply}) => {
                    if players.contains_key(&id) {
                        let _ = reply.send(Ok(()));
                        continue;
                    }

                    // if players.len() >= max_players { reply.send(Full) }
                    // state.add_player(id);
                    // send Joined msg to others
                    // send state to who joined

                    if let Ok(bytes) = encode_server(&ServerMsg::Welcome { your_id: id }) {
                        handle.reliable_tx.unbounded_send(bytes);
                        let spawn = Vec2::new(players.len() as f32 * 24.0, 0.);
                        physics.spawn_car(id, spawn);
                        players.insert(id, handle);
                        let _ = reply.send(Ok(()));
                    }
                    let _ = reply.send(Err(JoinError::Gone));
                }
                Some(LobbyCmd::Leave { id }) => {
                    players.remove(&id);
                    physics.remove_car(id);
                    // broadcast leave
                }
                Some(LobbyCmd::Msg {id, msg}) => {
                    match msg {
                        ClientMsg::Input { input } => {
                            physics.set_input(id, input);
                        }
                        _ => {}
                    }
                }
            },
            _ = tick.tick() => {
                tick_no = tick_no.wrapping_add(1);
                physics.step();

                let cars = physics.states().collect();
                let msg = ServerMsg::Snapshot { tick: tick_no, cars };
                if let Ok(bytes) = encode_server(&msg) {
                    for player in players.values() {
                        let _ = player.conn.send_datagram(bytes.clone());
                    }
                }
                // broadcast snapshots
            }
        }
    }
}

fn send_reject(handle: &PlayerHandle, reason: RejectReason) {
    if let Ok(bytes) = encode_server(&ServerMsg::Reject(reason)) {
        let _ = handle.reliable_tx.unbounded_send(bytes);
    }
}

fn code() -> String {
    format!("{:06}", rand::rng().random_range(0..1_000_000))
}

async fn read_reliable_stream(
    mut stream: RecvStream,
    tx: UnboundedSender<Vec<u8>>,
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
        tx.unbounded_send(buf)
            .map_err(|_| anyhow::anyhow!("recv queue closed"))?;
    }
}
