use std::{
    collections::HashMap,
    net::{IpAddr, SocketAddr},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering::Relaxed},
    },
    time::{Duration, Instant},
};

use axum::{
    Router,
    body::Bytes,
    extract::{
        ConnectInfo, State,
        ws::{Message, WebSocket, WebSocketUpgrade},
    },
    http::{HeaderMap, HeaderName, HeaderValue},
    response::IntoResponse,
    routing::get,
    serve::ListenerExt,
};
use futures_util::{SinkExt, StreamExt};
use rand::RngExt;
use resend_rs::{Resend, types::CreateEmailBaseOptions};
use tokio::sync::{Notify, mpsc, oneshot};
use tower_http::{services::ServeDir, set_header::SetResponseHeaderLayer};
use wrum_core::{
    PlayerId,
    level::Level,
    race::{RaceEvent, RaceProgress},
    sim::grid::Map,
    sim::{DT, Input, InputBuf, MAX_CARS, World},
};
use wrum_proto::*;

use crate::db::{Account, Db, EmailOutcome};

mod db;

const CLIENT_TIMEOUT: Duration = Duration::from_secs(45);
const RACE_AFK_TIMEOUT: Duration = Duration::from_secs(45);
const LOBBY_WAIT_TIMEOUT: Duration = Duration::from_secs(6 * 60);
const RESULTS_TIMEOUT: Duration = Duration::from_secs(20);
const COUNTDOWN_TICKS: u32 = 180;
const MIN_PLAYERS_PUBLIC: usize = 1;

// ---------------------------------------------------------------- shared state

struct Otp {
    hash: [u8; 32],
    expires: Instant,
    tries: u8,
    sent: Instant,
}

struct Shared {
    levels: HashMap<String, (Level, Map)>,
    db: Db,
    lobbies: Mutex<HashMap<String, LobbyHandle>>,
    online: Mutex<HashMap<i64, Arc<Notify>>>,
    otps: Mutex<HashMap<String, Otp>>,
    links: Mutex<HashMap<String, (i64, Instant)>>,
    limits: Mutex<HashMap<(IpAddr, &'static str), (Instant, u32)>>,
    resend: Resend,
    mail_from: String,
}

#[derive(Clone)]
struct LobbyHandle {
    tx: mpsc::UnboundedSender<Cmd>,
    count: Arc<AtomicUsize>,
    open: Arc<AtomicBool>,
    private: bool,
}

#[derive(Clone)]
struct Peer {
    tx: mpsc::Sender<Bytes>,
    kick: Arc<Notify>,
}
impl Peer {
    fn send(&self, b: &Bytes) {
        let _ = self.tx.try_send(b.clone());
    }
    fn send_fresh(&self, b: Bytes) {
        if self.tx.capacity() + 4 >= self.tx.max_capacity() {
            let _ = self.tx.try_send(b);
        }
    }
}
fn enc(m: &ServerMsg) -> Bytes {
    Bytes::from(encode_server(m).unwrap_or_default())
}

enum Cmd {
    Join {
        peer: Peer,
        name: String,
        country: [char; 2],
        account: Option<i64>,
        reply: oneshot::Sender<Result<u8, RejectReason>>,
    },
    Leave {
        slot: u8,
    },
    Msg {
        slot: u8,
        msg: ClientMsg,
    },
}

const LEVELS: [&str; 1] = [
    "assets/maps/untitled/untitled",
    //"assets/maps/beach/beach"
];

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let web = std::env::var("WEB").unwrap_or("web".into());
    let addr = std::env::var("ADDR").unwrap_or("0.0.0.0:8080".into());
    let key = std::env::var("RESEND_KEY")?;

    let db = Db::open(&std::env::var("DATABASE_URL")?).await?;
    let resend = Resend::new(&key);
    let mail_from = std::env::var("MAIL_FROM")?;

    let mut levels = HashMap::new();
    for l in LEVELS {
        let path = format!("{l}.json");
        println!("{path}");
        let level = Level::load(&path)?;
        let map = Map::from_level(&level);
        levels.insert(l.into(), (level, map));
    }

    let app = Arc::new(Shared {
        levels,
        db,
        lobbies: Default::default(),
        online: Default::default(),
        otps: Default::default(),
        links: Default::default(),
        limits: Default::default(),
        resend,
        mail_from,
    });

    let hdr = |k: &'static str, v: &'static str| {
        SetResponseHeaderLayer::overriding(HeaderName::from_static(k), HeaderValue::from_static(v))
    };
    let router = Router::new()
        .route("/ws", get(ws_handler))
        .fallback_service(ServeDir::new(web))
        .layer(hdr("cross-origin-opener-policy", "same-origin"))
        .layer(hdr("cross-origin-embedder-policy", "require-corp"))
        .with_state(app);

    println!("listening on {addr}");
    let listener = tokio::net::TcpListener::bind(addr).await?.tap_io(|tcp| {
        let _ = tcp.set_nodelay(true);
    });
    axum::serve(
        listener,
        router.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await?;
    Ok(())
}

async fn ws_handler(
    ws: WebSocketUpgrade,
    State(app): State<Arc<Shared>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let ip = headers
        .get("x-real-ip")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse().ok())
        .unwrap_or(addr.ip());
    ws.max_message_size(16 * 1024)
        .on_upgrade(move |s| ws_session(s, app, ip))
}

// ---------------------------------------------------------------- conn

fn new_token() -> (String, [u8; 32]) {
    let token = hex::encode(rand::rng().random::<[u8; 32]>());
    let hash = hash_token(&token);
    (token, hash)
}
fn hash_token(t: &str) -> [u8; 32] {
    *blake3::hash(t.as_bytes()).as_bytes()
}
fn allow(app: &Shared, ip: IpAddr, kind: &'static str, max: u32, secs: u64) -> bool {
    let mut m = app.limits.lock().unwrap();
    if m.len() > 50_000 {
        m.retain(|_, v| v.0.elapsed() < Duration::from_secs(3600));
    }
    let e = m.entry((ip, kind)).or_insert((Instant::now(), 0));
    if e.0.elapsed() > Duration::from_secs(secs) {
        *e = (Instant::now(), 0);
    }
    e.1 += 1;
    e.1 <= max
}
fn login(app: &Shared, id: i64, kick: &Arc<Notify>) {
    if let Some(old) = app.online.lock().unwrap().insert(id, kick.clone()) {
        old.notify_one();
    }
}

fn norm_email(e: &str) -> Option<String> {
    let e = e.trim().to_lowercase();
    let (l, d) = e.split_once('@')?;
    (e.len() <= 120
        && !l.is_empty()
        && d.contains('.')
        && !d.starts_with('.')
        && !d.ends_with('.')
        && !e.contains(char::is_whitespace)
        && e.matches('@').count() == 1)
        .then_some(e)
}
fn rand_code(n: usize, digits: bool) -> String {
    let al: &[u8] = if digits {
        b"0123456789"
    } else {
        b"ABCDEFGHJKMNPQRSTUVWXYZ23456789"
    };
    let mut r = rand::rng();
    (0..n)
        .map(|_| al[r.random_range(0..al.len())] as char)
        .collect()
}
fn check_otp(app: &Shared, email: &str, code: &str) -> Result<(), CodeError> {
    let mut o = app.otps.lock().unwrap();
    let Some(x) = o.get_mut(email) else {
        return Err(CodeError::Expired);
    };
    if x.expires < Instant::now() {
        o.remove(email);
        return Err(CodeError::Expired);
    }
    x.tries += 1;
    if x.tries > 5 {
        o.remove(email);
        return Err(CodeError::Expired);
    }
    if x.hash == hash_token(code.trim()) {
        o.remove(email);
        Ok(())
    } else {
        Err(CodeError::Wrong)
    }
}

async fn send_mail(app: Arc<Shared>, to: String, code: String) {
    let email = CreateEmailBaseOptions::new(&app.mail_from, [to], "wrum login code")
        .with_text(&format!("Your code is {code}. Expires in 10 minutes"));
    match app.resend.emails.send(email).await {
        Err(e) => eprintln!("mail: {e}"),
        _ => {}
    }
}

async fn session(mut inbox: mpsc::Receiver<Bytes>, me: Peer, app: Arc<Shared>, ip: IpAddr) {
    let kick = me.kick.clone();

    let mut name = format!("guest-{:04}", rand::rng().random_range(0..10_000));
    let mut country = ['F', 'I'];
    let mut acct: Option<i64> = None;
    let mut hello = false;
    let mut last_board = Instant::now() - Duration::from_secs(1);
    let mut cur: Option<(mpsc::UnboundedSender<Cmd>, u8)> = None;
    let mut cur_hash: Option<[u8; 32]> = None;
    let mut verified_email: Option<String> = None;
    let reply = |m: ServerMsg| me.send(&enc(&m));

    macro_rules! logged_in {
        ($a:expr, $token:expr, $hash:expr) => {{
            let a: Account = $a;
            leave(&mut cur);
            login(&app, a.id, &kick);
            acct = Some(a.id);
            name = a.name.clone();
            country = a.country;
            cur_hash = Some($hash);
            verified_email = None;
            reply(ServerMsg::LoggedIn {
                token: $token,
                name: a.name,
                country: a.country,
                email: a.email,
            });
        }};
    }

    loop {
        let limit = if hello {
            CLIENT_TIMEOUT
        } else {
            Duration::from_secs(5)
        };
        let b = tokio::select! {
            _ = kick.notified() => break,
            r = tokio::time::timeout(limit, inbox.recv()) => match r {
                Ok(Some(b)) => b,
                _ => break,
            },
        };
        let Ok(msg) = decode_client(&b) else { continue };

        match msg {
            ClientMsg::Hello { version, token } => {
                if version != PROTO_VERSION {
                    reply(ServerMsg::Reject(RejectReason::VersionMismatch));
                    break;
                }
                hello = true;
                let mut email = None;
                if let Some(t) = token {
                    let h = hash_token(&t);
                    if let Some(a) = app.db.auth(h).await {
                        login(&app, a.id, &kick);
                        acct = Some(a.id);
                        name = a.name;
                        country = a.country;
                        email = a.email;
                        cur_hash = Some(h);
                    }
                }
                reply(ServerMsg::HelloOk {
                    name: name.clone(),
                    country,
                    guest: acct.is_none(),
                    email,
                });
            }
            _ if !hello => {}

            ClientMsg::Heartbeat => {}
            ClientMsg::Register {
                country: wanted_country,
                name: wanted,
            } if acct.is_none() => {
                if !allow(&app, ip, "reg", 5, 3600) {
                    reply(ServerMsg::NameRejected(NameError::Busy));
                    continue;
                }
                let Some(n) = valid_name(&wanted) else {
                    reply(ServerMsg::NameRejected(NameError::Invalid));
                    continue;
                };
                let (token, hash) = new_token();
                match app
                    .db
                    .register(n.clone(), wanted_country, verified_email.clone(), hash)
                    .await
                {
                    Ok(id) => {
                        login(&app, id, &kick);
                        acct = Some(id);
                        name = n.clone();
                        country = wanted_country;
                        reply(ServerMsg::Registered {
                            token,
                            name: n,
                            country,
                        });
                        verified_email = None;
                    }
                    Err(e) => reply(ServerMsg::NameRejected(e)),
                }
            }
            ClientMsg::Update {
                country: wanted_country,
                name: wanted,
            } if let Some(id) = acct => {
                let Some(n) = valid_name(&wanted) else {
                    me.send(&enc(&ServerMsg::NameRejected(NameError::Invalid)));
                    continue;
                };
                match app.db.update(id, n.clone(), wanted_country).await {
                    Ok(()) => {
                        name = n.clone();
                        country = wanted_country;
                        me.send(&enc(&ServerMsg::Updated { name: n, country }));
                    }
                    Err(e) => me.send(&enc(&ServerMsg::NameRejected(e))),
                }
            }
            ClientMsg::GetLeaderboard { kind } => {
                if last_board.elapsed() < Duration::from_secs(1) {
                    continue;
                }
                last_board = Instant::now();
                me.send(&enc(&app
                    .db
                    .board(format!("{}.json", LEVELS[0]), kind, acct)
                    .await));
            }

            ClientMsg::RequestCode { email } => {
                let Some(e) = norm_email(&email) else {
                    reply(ServerMsg::CodeRejected(CodeError::BadEmail));
                    continue;
                };
                if !allow(&app, ip, "mail", 8, 3600) {
                    reply(ServerMsg::CodeRejected(CodeError::Busy));
                    continue;
                }
                let code = rand_code(6, true);
                {
                    let mut o = app.otps.lock().unwrap();
                    o.retain(|_, x| x.expires > Instant::now());
                    if o.get(&e)
                        .is_some_and(|x| x.sent.elapsed() < Duration::from_secs(60))
                    {
                        drop(o);
                        reply(ServerMsg::CodeRejected(CodeError::TooSoon));
                        continue;
                    }
                    o.insert(
                        e.clone(),
                        Otp {
                            hash: hash_token(&code),
                            expires: Instant::now() + Duration::from_secs(600),
                            tries: 0,
                            sent: Instant::now(),
                        },
                    );
                }
                tokio::spawn(send_mail(app.clone(), e, code));
                reply(ServerMsg::CodeSent);
            }
            ClientMsg::VerifyCode { email, code } => {
                let Some(e) = norm_email(&email) else {
                    reply(ServerMsg::CodeRejected(CodeError::BadEmail));
                    continue;
                };
                if !allow(&app, ip, "verify", 30, 3600) {
                    reply(ServerMsg::CodeRejected(CodeError::Busy));
                    continue;
                }
                if let Err(er) = check_otp(&app, &e, &code) {
                    reply(ServerMsg::CodeRejected(er));
                    continue;
                }
                let (token, hash) = new_token();
                match app.db.email_verified(&e, acct, hash).await {
                    Ok(EmailOutcome::Attached) => reply(ServerMsg::EmailLinked { email: e }),
                    Ok(EmailOutcome::LoggedInto(a)) => logged_in!(a, token, hash),
                    Ok(EmailOutcome::NeedName) => {
                        verified_email = Some(e);
                        reply(ServerMsg::NeedName);
                    }
                    Err(er) => {
                        eprintln!("email_verified: {er}");
                        reply(ServerMsg::CodeRejected(CodeError::Busy));
                    }
                }
            }
            ClientMsg::MakeLinkCode if acct.is_some() => {
                let code = rand_code(8, false);
                {
                    let mut l = app.links.lock().unwrap();
                    l.retain(|_, (_, exp)| *exp > Instant::now());
                    l.insert(
                        code.clone(),
                        (acct.unwrap(), Instant::now() + Duration::from_secs(300)),
                    );
                }
                reply(ServerMsg::LinkCode { code });
            }
            ClientMsg::RedeemLinkCode { code } => {
                if !allow(&app, ip, "link", 20, 3600) {
                    reply(ServerMsg::CodeRejected(CodeError::Busy));
                    continue;
                }
                let hit = app
                    .links
                    .lock()
                    .unwrap()
                    .remove(&code.trim().to_uppercase());
                let Some((id, _)) = hit.filter(|&(_, exp)| exp > Instant::now()) else {
                    reply(ServerMsg::CodeRejected(CodeError::Wrong));
                    continue;
                };
                let (token, hash) = new_token();
                match app.db.add_device(id, hash).await {
                    Some(a) => logged_in!(a, token, hash),
                    None => reply(ServerMsg::CodeRejected(CodeError::Busy)),
                }
            }

            ClientMsg::ListDevices if acct.is_some() => {
                reply(ServerMsg::Devices {
                    list: app.db.devices(acct.unwrap(), cur_hash).await,
                });
            }
            ClientMsg::RevokeDevice { id } if acct.is_some() => {
                app.db.revoke_device(acct.unwrap(), &id).await;
                reply(ServerMsg::Devices {
                    list: app.db.devices(acct.unwrap(), cur_hash).await,
                });
            }
            ClientMsg::Logout { everywhere } if acct.is_some() => {
                if everywhere {
                    app.db.logout_all(acct.unwrap()).await;
                } else if let Some(h) = cur_hash {
                    app.db.logout_device(h).await;
                }
                reply(ServerMsg::LoggedOut);
                tokio::time::sleep(Duration::from_millis(100)).await;
                break;
            }
            ClientMsg::DeleteAccount if acct.is_some() => {
                app.db.delete_account(acct.unwrap()).await;
                reply(ServerMsg::LoggedOut);
                tokio::time::sleep(Duration::from_millis(100)).await;
                break;
            }

            // lobby
            ClientMsg::CreateLobby => {
                leave(&mut cur);
                cur = join(&app, &me, &name, country, acct, Target::Create).await;
            }
            ClientMsg::JoinLobby { code } => {
                leave(&mut cur);
                cur = join(&app, &me, &name, country, acct, Target::Code(code)).await;
            }
            ClientMsg::JoinPublic => {
                leave(&mut cur);
                cur = join(&app, &me, &name, country, acct, Target::Public).await;
            }
            ClientMsg::LeaveLobby => leave(&mut cur),
            msg => {
                if let Some((tx, slot)) = &cur {
                    if tx.send(Cmd::Msg { slot: *slot, msg }).is_err() {
                        cur = None;
                        me.send(&enc(&ServerMsg::Reject(RejectReason::LobbyGone)));
                    }
                }
            }
        }
    }
    leave(&mut cur);
    if let Some(id) = acct {
        let mut on = app.online.lock().unwrap();
        if on.get(&id).is_some_and(|k| Arc::ptr_eq(k, &kick)) {
            on.remove(&id);
        }
    }
}
async fn ws_session(socket: WebSocket, app: Arc<Shared>, ip: IpAddr) {
    let (mut sink, mut stream) = socket.split();
    let (tx, mut rx) = mpsc::channel::<Bytes>(64);
    let (in_tx, in_rx) = mpsc::channel::<Bytes>(256);
    let me = Peer {
        tx,
        kick: Arc::new(Notify::new()),
    };
    let w = tokio::spawn(async move {
        while let Some(b) = rx.recv().await {
            if sink.send(Message::Binary(b)).await.is_err() {
                break;
            }
        }
    });
    let r = tokio::spawn(async move {
        while let Some(Ok(m)) = stream.next().await {
            if let Message::Binary(b) = m {
                if in_tx.send(b).await.is_err() {
                    break;
                }
            }
        }
    });
    session(in_rx, me, app, ip).await;
    w.abort();
    r.abort();
}

fn leave(cur: &mut Option<(mpsc::UnboundedSender<Cmd>, u8)>) {
    if let Some((tx, slot)) = cur.take() {
        let _ = tx.send(Cmd::Leave { slot });
    }
}

enum Target {
    Create,
    Code(String),
    Public,
}

fn random_level(app: &Shared) -> &str {
    let mut rng = rand::rng();
    let i = rng.random_range(0..app.levels.len());
    app.levels.keys().nth(i).unwrap()
}

async fn join(
    app: &Arc<Shared>,
    me: &Peer,
    name: &str,
    country: [char; 2],
    account: Option<i64>,
    t: Target,
) -> Option<(mpsc::UnboundedSender<Cmd>, u8)> {
    let reject = |r| me.send(&enc(&ServerMsg::Reject(r)));
    let handle = match t {
        // todo: Target::Create(level_id)
        Target::Create => create_lobby(app, true, random_level(app)),
        Target::Code(c) => match find(app, |k, _| k == c) {
            Some((_, h)) => h,
            None => {
                reject(RejectReason::LobbyNotFound);
                return None;
            }
        },
        Target::Public => match find(app, |_, h| {
            !h.private && h.open.load(Relaxed) && h.count.load(Relaxed) < MAX_CARS
        }) {
            Some((_, h)) => h,
            None => create_lobby(app, false, random_level(app)),
        },
    };
    let (reply, rx) = oneshot::channel();
    if handle
        .tx
        .send(Cmd::Join {
            peer: me.clone(),
            name: name.into(),
            country,
            account,
            reply,
        })
        .is_err()
    {
        reject(RejectReason::LobbyGone);
        return None;
    }
    match rx.await {
        Ok(Ok(slot)) => Some((handle.tx, slot)),
        Ok(Err(r)) => {
            reject(r);
            None
        }
        Err(_) => {
            reject(RejectReason::LobbyGone);
            None
        }
    }
}

fn find(app: &Shared, f: impl Fn(&str, &LobbyHandle) -> bool) -> Option<(String, LobbyHandle)> {
    let mut map = app.lobbies.lock().unwrap();
    map.retain(|_, h| !h.tx.is_closed());
    map.iter()
        .find(|(k, h)| f(k, h))
        .map(|(k, h)| (k.clone(), h.clone()))
}
fn create_lobby(app: &Arc<Shared>, private: bool, level_id: &str) -> LobbyHandle {
    let mut map = app.lobbies.lock().unwrap();
    let code = loop {
        let c = format!("{:06}", rand::rng().random_range(0..1_000_000));
        if !map.contains_key(&c) {
            break c;
        }
    };
    let (tx, rx) = mpsc::unbounded_channel();
    let handle = LobbyHandle {
        tx,
        count: Arc::new(AtomicUsize::new(0)),
        open: Arc::new(AtomicBool::new(true)),
        private,
    };

    map.insert(code.clone(), handle.clone());

    let (level, map) = app.levels.get(level_id).expect("unknown level");
    let lobby = Lobby::new(
        app.clone(),
        code,
        handle.clone(),
        level_id.to_owned(),
        level.clone(),
        map.clone(),
    );

    tokio::spawn(lobby.run(rx));
    handle
}

// ---------------------------------------------------------------- lobby

#[derive(Clone, Copy, PartialEq)]
enum Phase {
    Waiting,
    Countdown { go_tick: u32 },
    Racing,
    Results { reset_at: Instant },
}

struct Slot {
    peer: Peer,
    name: String,
    country: [char; 2],
    inbuf: InputBuf,
    progress: RaceProgress,
    last_control: Instant,
    early: i8,
    ready: bool,
    account: Option<i64>,
    recorded: bool,
}

struct Lobby {
    app: Arc<Shared>,
    level_id: String,
    level: Level,
    map: Map,
    code: String,
    h: LobbyHandle,
    world: World,
    slots: [Option<Slot>; MAX_CARS],
    phase: Phase,
    host: u8,
    tick_at: Instant,
    waiting_since: Instant,
}

impl Lobby {
    fn new(
        app: Arc<Shared>,
        code: String,
        h: LobbyHandle,
        level_id: String,
        level: Level,
        map: Map,
    ) -> Self {
        Self {
            app,
            code,
            h,
            level_id,
            level,
            map,
            world: World {
                tick: 600,
                ..Default::default()
            },
            slots: std::array::from_fn(|_| None),
            phase: Phase::Waiting,
            host: 0,
            tick_at: Instant::now(),
            waiting_since: Instant::now(),
        }
    }

    async fn run(mut self, mut rx: mpsc::UnboundedReceiver<Cmd>) {
        let mut tick = tokio::time::interval(Duration::from_secs_f64(1.0 / 60.0));
        tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Burst);
        'outer: loop {
            tokio::select! {
                biased;
                _ = tick.tick() => {
                    while let Ok(c) = rx.try_recv() {
                        if self.on_cmd(c) { break 'outer; }
                    }
                    self.tick();
                    if self.phase == Phase::Waiting && self.waiting_since.elapsed() > LOBBY_WAIT_TIMEOUT { break }
                }
                cmd = rx.recv() => match cmd {
                    None => break,
                    Some(c) => if self.on_cmd(c) { break },
                },
            }
        }
        self.h.open.store(false, Relaxed);
        self.broadcast(&ServerMsg::Reject(RejectReason::LobbyGone));
        self.app.lobbies.lock().unwrap().remove(&self.code);
        println!("closed lobby {}", self.code);
    }

    fn players(&self) -> impl Iterator<Item = (usize, &Slot)> {
        self.slots
            .iter()
            .enumerate()
            .filter_map(|(i, s)| s.as_ref().map(|s| (i, s)))
    }
    fn count(&self) -> usize {
        self.players().count()
    }
    fn broadcast(&self, m: &ServerMsg) {
        let b = enc(m);
        for (_, s) in self.players() {
            s.peer.send(&b);
        }
    }
    fn lobby_state(&self) -> ServerMsg {
        ServerMsg::LobbyState {
            host: self.host,
            ready: self
                .players()
                .filter(|(_, s)| s.ready)
                .map(|(i, _)| i as u8)
                .collect(),
        }
    }

    fn on_cmd(&mut self, cmd: Cmd) -> bool {
        match cmd {
            Cmd::Join {
                peer,
                name,
                country,
                account,
                reply,
            } => {
                let _ = reply.send(self.join(peer, name, country, account));
            }
            Cmd::Leave { slot } => {
                let i = slot as usize;
                if self.phase == Phase::Racing {
                    let of = self.count() as u32;
                    self.record_slot(i, of, of);
                }
                if self.slots[i].take().is_none() {
                    return false;
                }
                self.world.remove(i);
                self.h.count.store(self.count(), Relaxed);
                if self.count() == 0 {
                    return true;
                }
                if self.host == slot {
                    let next_host = self.players().next().map(|(i, _)| i as u8).unwrap_or(0);
                    self.host = next_host;
                }
                self.broadcast(&ServerMsg::PlayerLeft { slot });
                self.broadcast(&self.lobby_state());
                if self.phase == Phase::Waiting && self.all_ready() {
                    self.begin_countdown();
                }
                if self.phase == Phase::Racing && self.everyone_finished() {
                    self.finish_race();
                }
            }
            Cmd::Msg { slot, msg } => self.on_msg(slot as usize, msg),
        }
        false
    }
    fn join(
        &mut self,
        peer: Peer,
        name: String,
        country: [char; 2],
        account: Option<i64>,
    ) -> Result<u8, RejectReason> {
        if self.phase != Phase::Waiting {
            return Err(RejectReason::RaceStarted);
        }
        let Some(i) = self.slots.iter().position(Option::is_none) else {
            return Err(RejectReason::LobbyFull);
        };

        let (pos, rot) = self.level.spawns[i % self.level.spawns.len()];
        self.world.spawn(i, pos, rot);
        let mut inbuf = InputBuf::default();
        inbuf.reset(self.world.tick);
        if self.count() == 0 {
            self.host = i as u8;
        }
        self.slots[i] = Some(Slot {
            peer: peer.clone(),
            name: name.clone(),
            country,
            account,
            inbuf,
            progress: RaceProgress::new(self.world.tick),
            last_control: Instant::now(),
            early: 127,
            ready: false,
            recorded: false,
        });
        self.h.count.store(self.count(), Relaxed);
        self.waiting_since = Instant::now();

        let players = self
            .players()
            .map(|(i, s)| (i as u8, s.name.clone(), s.country))
            .collect();
        peer.send(&enc(&ServerMsg::Welcome {
            slot: i as u8,
            tick: self.world.tick,
            code: self.code.clone(),
            private: self.h.private,
            players,
            map: self.level_id.clone(),
        }));
        let joined = enc(&ServerMsg::PlayerJoined {
            slot: i as u8,
            name,
            country,
        });
        for (j, s) in self.players() {
            if j != i {
                s.peer.send(&joined);
            }
        }
        self.broadcast(&self.lobby_state());
        Ok(i as u8)
    }
    fn on_msg(&mut self, i: usize, msg: ClientMsg) {
        match msg {
            ClientMsg::Ping { t0 } => {
                let frac = (self.tick_at.elapsed().as_secs_f32() / DT).min(0.999);
                if let Some(s) = &self.slots[i] {
                    s.peer.send(&enc(&ServerMsg::Pong {
                        t0,
                        tick: self.world.tick,
                        frac,
                    }));
                }
            }
            ClientMsg::Inputs { first, inputs } => self.on_inputs(i, first, inputs),
            ClientMsg::Ready(r) if self.phase == Phase::Waiting && !self.h.private => {
                if let Some(s) = &mut self.slots[i] {
                    s.ready = r;
                }
                self.broadcast(&self.lobby_state());
                if self.all_ready() {
                    self.begin_countdown();
                }
            }
            ClientMsg::StartRace
                if self.phase == Phase::Waiting && self.h.private && self.host as usize == i =>
            {
                self.begin_countdown();
            }
            ClientMsg::PlayAgain if matches!(self.phase, Phase::Results { .. }) => {
                if let Some(s) = &mut self.slots[i] {
                    s.ready = true;
                }
                self.broadcast(&self.lobby_state());
                if self.players().all(|(_, s)| s.ready) {
                    self.reset_race();
                    self.begin_countdown();
                }
            }
            _ => {}
        }
    }
    fn on_inputs(&mut self, i: usize, first: u32, inputs: Vec<Input>) {
        let now = self.world.tick;
        let skip = inputs.len().saturating_sub(16);
        let inputs = inputs[skip..].to_vec();
        let first = first.wrapping_add(skip as u32);
        let n = inputs.len();
        if n == 0 {
            return;
        }
        let Some(s) = &mut self.slots[i] else { return };

        if inputs
            .iter()
            .any(|x| x.throttle != 0 || x.steer != 0 || x.drift)
        {
            s.last_control = Instant::now();
        }
        let e = first.wrapping_add(n as u32 - 1).wrapping_sub(now) as i32;
        s.early = s.early.min(e.clamp(-127, 126) as i8);

        let mut first_new = None;
        for (k, inp) in inputs[..n].iter().enumerate() {
            let t = first.wrapping_add(k as u32);
            let d = t.wrapping_sub(now) as i32;
            if d < 0 || d > 90 {
                continue;
            }
            if s.inbuf.push(t, *inp) && first_new.is_none() {
                first_new = Some(k);
            }
        }
        let tail = s.inbuf.tail(4);
        if first_new.is_some() {
            let msg = enc(&ServerMsg::Relay {
                slot: i as u8,
                first: tail.0,
                inputs: tail.1,
            });
            for (j, o) in self.players() {
                if j != i {
                    o.peer.send(&msg);
                }
            }
        }
    }

    // -------- tick

    fn tick(&mut self) {
        if self.count() == 0 {
            return;
        }
        let t = self.world.tick;
        match self.phase {
            Phase::Countdown { go_tick } if (t.wrapping_sub(go_tick) as i32) >= 0 => {
                self.phase = Phase::Racing;
                for s in self.slots.iter_mut().flatten() {
                    s.progress = RaceProgress::new(t);
                    s.last_control = Instant::now();
                }
            }
            Phase::Results { reset_at } if Instant::now() >= reset_at => {
                self.reset_race();
                self.h.open.store(true, Relaxed);
            }
            _ => {}
        }

        let mut ins = [Input::default(); MAX_CARS];
        if matches!(self.phase, Phase::Racing | Phase::Results { .. }) {
            for (i, s) in self.players() {
                ins[i] = s.inbuf.at(t);
            }
        }
        let mut crossed: Vec<(usize, u32)> = Vec::new();
        self.world
            .step(&self.map, &ins, true, &mut |i, g| crossed.push((i, g)));
        self.tick_at = Instant::now();
        let now = self.world.tick;

        if self.phase == Phase::Racing {
            let n_cp = self.level.checkpoint_count;
            let mut msgs = Vec::new();
            let mut finished_now = false;
            let mut finished_slots: Vec<usize> = Vec::new();

            for (i, s) in self
                .slots
                .iter_mut()
                .enumerate()
                .filter_map(|(i, s)| s.as_mut().map(|s| (i, s)))
            {
                let touching: Vec<u32> = crossed.iter().filter(|c| c.0 == i).map(|c| c.1).collect();
                let Some(ev) = s.progress.update(&touching, n_cp, now) else {
                    continue;
                };
                if matches!(ev, RaceEvent::Finished { .. }) {
                    finished_slots.push(i);
                }
                finished_now |= matches!(ev, RaceEvent::Finished { .. });
                let lap_time = match ev {
                    RaceEvent::Lap { lap_time } | RaceEvent::Finished { lap_time } => {
                        Some(lap_time)
                    }
                    RaceEvent::Checkpoint => None,
                };
                msgs.push(ServerMsg::Race {
                    slot: i as u8,
                    lap: s.progress.lap,
                    next: s.progress.next,
                    finished: s.progress.finished,
                    lap_time,
                });
            }
            for m in &msgs {
                self.broadcast(m);
            }
            for i in finished_slots {
                let of = self.count() as u32;
                let pos = self.players().filter(|(_, s)| s.recorded).count() as u32 + 1;
                self.record_slot(i, pos, of);
            }
            if finished_now && self.everyone_finished() {
                self.finish_race();
            }
            if now % 120 == 0 {
                for (_, s) in self.players() {
                    if !s.progress.finished && s.last_control.elapsed() > RACE_AFK_TIMEOUT {
                        s.peer.kick.notify_one();
                    }
                }
            }
        }

        if now % SNAPSHOT_EVERY == 0 {
            let world = self.world;
            for s in self.slots.iter_mut().flatten() {
                s.peer.send_fresh(enc(&ServerMsg::Snapshot {
                    world,
                    early: s.early,
                }));
                s.early = 127;
            }
        }
        if now % 6 == 0 {
            self.broadcast(&ServerMsg::Positions {
                order: self.ranking(),
            });
        }
    }

    // -------- race

    fn all_ready(&self) -> bool {
        !self.h.private
            && self.count() >= MIN_PLAYERS_PUBLIC
            && self.players().all(|(_, s)| s.ready)
    }
    fn everyone_finished(&self) -> bool {
        self.count() > 0 && self.players().all(|(_, s)| s.progress.finished)
    }
    fn begin_countdown(&mut self) {
        let go_tick = self.world.tick.wrapping_add(COUNTDOWN_TICKS);
        self.phase = Phase::Countdown { go_tick };
        self.h.open.store(false, Relaxed);
        self.broadcast(&ServerMsg::Countdown { go_tick });
    }
    fn finish_race(&mut self) {
        let order = self.ranking();

        let players = order
            .iter()
            .filter_map(|&id| {
                let p = &self.slots[id as usize].as_ref()?.progress;
                Some(PlayerResult {
                    id,
                    lap_times: p.lap_times.clone(),
                    total_time: p.lap_times.iter().sum(),
                    finished: p.finished,
                })
            })
            .collect();
        let reset_tick = self
            .world
            .tick
            .wrapping_add((RESULTS_TIMEOUT.as_secs_f64() / DT as f64).ceil() as u32);
        self.phase = Phase::Results {
            reset_at: Instant::now() + RESULTS_TIMEOUT,
        };

        self.h.open.store(false, Relaxed);
        self.broadcast(&ServerMsg::Results {
            order: order.clone(),
            players,
            reset_tick,
        });

        let of = order.len() as u32;
        for (i, &id) in order.iter().enumerate() {
            self.record_slot(id as usize, i as u32 + 1, of);
        }
    }
    fn reset_race(&mut self) {
        let tick = self.world.tick;
        self.world = World {
            tick,
            ..Default::default()
        };
        let spawns = &self.level.spawns;
        for (i, s) in self.slots.iter_mut().enumerate() {
            let Some(s) = s else { continue };
            let (pos, rot) = spawns[i % spawns.len()];
            self.world.spawn(i, pos, rot);
            s.inbuf = InputBuf::default();
            s.inbuf.reset(tick);
            s.progress = RaceProgress::new(tick);
            s.ready = false;
        }
        self.phase = Phase::Waiting;
        self.waiting_since = Instant::now();
        self.broadcast(&ServerMsg::RaceReset);
        self.broadcast(&self.lobby_state());
    }

    fn ranking(&self) -> Vec<PlayerId> {
        let n = self.level.checkpoint_count;
        let mut v: Vec<_> = self
            .players()
            .map(|(i, s)| {
                let p = &s.progress;
                let d = self.level.dist_to_gate(p.next, self.world.cars[i].pos);
                (i as u8, p.finish_tick, p.score(n), d)
            })
            .collect();
        use std::cmp::Ordering::*;
        v.sort_by(|a, b| match (a.1, b.1) {
            (Some(x), Some(y)) => x.cmp(&y).then(a.0.cmp(&b.0)),
            (Some(_), None) => Less,
            (None, Some(_)) => Greater,
            (None, None) => {
                b.2.cmp(&a.2)
                    .then(a.3.partial_cmp(&b.3).unwrap_or(Equal))
                    .then(a.0.cmp(&b.0))
            }
        });
        v.into_iter().map(|t| t.0).collect()
    }
    fn record_slot(&mut self, i: usize, pos: u32, of: u32) {
        let Some(s) = self.slots[i].as_mut() else {
            return;
        };
        if s.recorded {
            return;
        }
        s.recorded = true;
        let Some(account) = s.account else { return };
        let p = &s.progress;
        let best_lap = p.lap_times.iter().copied().reduce(f32::min);
        let total = p.finished.then(|| p.lap_times.iter().sum::<f32>());
        eprintln!("record: account {account} pos {pos}/{of} total {total:?} lap {best_lap:?}");
        self.app
            .db
            .record(account, self.level_id.clone(), pos, of, total, best_lap);
    }
}
