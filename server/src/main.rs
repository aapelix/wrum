use std::{
    collections::{HashMap, VecDeque},
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, AtomicUsize, Ordering},
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
    PlayerId, TICK_DT,
    level::Level,
    physics::{Input, PhysicsWorld},
    race::{RaceEvent, RaceProgress},
};
use wrum_net::{
    MAX_PACKET_SIZE,
    proto::{ClientMsg, RejectReason, ServerMsg, decode_client, encode_server},
};
use wtransport::{
    Connection, Endpoint, Identity, RecvStream, ServerConfig, endpoint::IncomingSession,
};

static NEXT_ID: AtomicU64 = AtomicU64::new(0);

const MAX_PLAYERS: usize = 8;
const TARGET_DEPTH: usize = 2;
const MAX_DEPTH: usize = 6;

#[derive(Default)]
struct InputBuffer {
    queue: VecDeque<Input>,
    newest_seq: u32,
    has_any: bool,
    last: Input,
    primed: bool,
}
impl InputBuffer {
    fn push(&mut self, inputs: &[Input]) {
        for &i in inputs {
            if !self.has_any || (i.seq.wrapping_sub(self.newest_seq) as i32) > 0 {
                self.newest_seq = i.seq;
                self.has_any = true;
                self.queue.push_back(i);
            }
        }
    }
    fn next(&mut self) -> Input {
        if self.queue.len() > MAX_DEPTH {
            while self.queue.len() > TARGET_DEPTH {
                self.queue.pop_front();
            }
        }
        if !self.primed {
            if self.queue.len() < TARGET_DEPTH {
                return self.last;
            }
            self.primed = true;
        }
        match self.queue.pop_front() {
            Some(i) => {
                self.last = i;
                i
            }
            None => {
                self.primed = false;
                self.last
            }
        }
    }
}

#[derive(Clone)]
struct PlayerHandle {
    conn: Connection,
    reliable_tx: UnboundedSender<Vec<u8>>,
}

#[derive(Clone)]
struct LobbyHandle {
    tx: UnboundedSender<LobbyCmd>,
    players: Arc<AtomicUsize>,
    private: bool,
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

type Lobbies = Arc<Mutex<HashMap<String, LobbyHandle>>>;

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
                let msg = match decode_client(&bytes) {
                    Ok(m) => m,
                    Err(e) => { eprintln!("decode_client failed ({} bytes): {e}", bytes.len()); continue; }
                };

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
                    ClientMsg::JoinPublic => {
                        leave_current(&mut current, id);
                        match find_public_lobby(&lobbies) {
                            Some((lid, tx)) => {
                                join(&mut current, lid, tx, id, &me).await;
                            }
                            None => {
                                let (lid, tx) = create_lobby(&lobbies, id);
                                join(&mut current, lid, tx, id, &me).await;
                            }
                        }
                    }
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
    let players = Arc::new(AtomicUsize::new(0));

    tokio::spawn(run_lobby(rx, host, players.clone()));
    l.lock().unwrap().insert(
        lid.clone(),
        LobbyHandle {
            tx: tx.clone(),
            players,
            private: false,
        },
    );
    (lid, tx)
}

fn find_lobby(l: &Lobbies, id: &str) -> Option<UnboundedSender<LobbyCmd>> {
    let mut map = l.lock().unwrap();
    match map.get(id) {
        Some(handle) if !handle.tx.is_closed() => Some(handle.tx.clone()),
        Some(_) => {
            map.remove(id);
            None
        }
        None => None,
    }
}
fn find_public_lobby(l: &Lobbies) -> Option<(String, UnboundedSender<LobbyCmd>)> {
    let mut map = l.lock().unwrap();
    map.retain(|_, handle| !handle.tx.is_closed());

    map.iter()
        .find(|(_, handle)| !handle.private && handle.players.load(Ordering::Relaxed) < MAX_PLAYERS)
        .map(|(id, handle)| (id.clone(), handle.tx.clone()))
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

async fn run_lobby(
    mut rx: UnboundedReceiver<LobbyCmd>,
    _host: u64,
    player_count: Arc<AtomicUsize>,
) {
    let level = Level::load("assets/maps/untitled.json").unwrap_or_else(|e| {
        eprintln!("failed to load level: {e:#?}");
        Level::default()
    });
    let n_cp = level.checkpoints.len() as u32;

    let mut players: HashMap<PlayerId, PlayerHandle> = HashMap::new();
    let mut physics = PhysicsWorld::default();
    physics.load_level(&level);
    let mut buffers: HashMap<PlayerId, InputBuffer> = HashMap::new();
    let mut progress: HashMap<PlayerId, RaceProgress> = HashMap::new();

    let mut tick = tokio::time::interval(Duration::from_secs_f64(TICK_DT as f64));
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
                    if players.len() >= MAX_PLAYERS {
                        let _ = reply.send(Err(JoinError::Full));
                        continue;
                    }

                    let existing = physics.states().collect();
                    let (spawn_p, spawn_a) = level.spawns[players.len() % level.spawns.len()];
                    physics.spawn_car(id, spawn_p, spawn_a);
                    buffers.insert(id, InputBuffer::default());
                    progress.insert(id, RaceProgress::new(tick_no));

                    let spawn = physics.state(id).unwrap_or_default();
                    let msg = ServerMsg::PlayerJoined { id, name: "test".into(), spawn};
                    if let Ok(bytes) = encode_server(&msg) {
                        for player in players.values() {
                            let _ = player.conn.send_datagram(bytes.clone());
                        }
                    }

                    players.insert(id, handle);
                    player_count.store(players.len(), Ordering::Relaxed);
                    if let Ok(bytes) = encode_server(&ServerMsg::Welcome { your_id: id, spawn, existing }) {
                        let _ = players[&id].reliable_tx.unbounded_send(bytes);
                    }


                    let _ = reply.send(Ok(()));
                }
                Some(LobbyCmd::Leave { id }) => {
                    players.remove(&id);
                    buffers.remove(&id);
                    progress.remove(&id);
                    physics.remove_car(id);
                    let msg = ServerMsg::PlayerLeft { id };
                    if let Ok(bytes) = encode_server(&msg) {
                        for player in players.values() {
                            let _ = player.conn.send_datagram(bytes.clone());
                        }
                    }
                }
                Some(LobbyCmd::Msg {id, msg}) => {
                    match msg {
                        ClientMsg::Input { inputs } => {
                            if let Some(buf) = buffers.get_mut(&id) {
                                buf.push(&inputs[..inputs.len().min(8)]);
                            }
                        }
                        _ => {}
                    }
                }
            },
            _ = tick.tick() => {
                tick_no = tick_no.wrapping_add(1);
                for (id, buf) in buffers.iter_mut() {
                    physics.set_input(*id, buf.next());
                }
                physics.step();
                for (&id, p) in progress.iter_mut() {
                    let touching = physics.checkpoints_touching(id);
                    let Some(ev) = p.update(&touching, n_cp, tick_no) else { continue };
                    let lap_time = match ev {
                        RaceEvent::Lap { lap_time } | RaceEvent::Finished { lap_time } => Some(lap_time),
                        RaceEvent::Checkpoint => None,
                    };
                    let msg = ServerMsg::Race { id, lap: p.lap, next: p.next, finished: p.finished, lap_time };
                    if let Ok(bytes) = encode_server(&msg) {
                        for player in players.values() {
                            let _ = player.reliable_tx.unbounded_send(bytes.clone());
                        }
                    }
                }

                let cars = physics.states().collect();
                let msg = ServerMsg::Snapshot { tick: tick_no, cars };
                if let Ok(bytes) = encode_server(&msg) {
                    for player in players.values() {
                        let _ = player.conn.send_datagram(bytes.clone());
                    }
                }
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
