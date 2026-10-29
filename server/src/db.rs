use rusqlite::{Connection, ErrorCode, OptionalExtension, params};
use tokio::sync::{mpsc, oneshot};
use wrum_proto::{BoardKind, NameError, ServerMsg};

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS players (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL UNIQUE COLLATE NOCASE,
    country TEXT NOT NULL,
    token_hash BLOB NOT NULL UNIQUE,
    created_at INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS results (
    id INTEGER PRIMARY KEY,
    player_id INTEGER NOT NULL REFERENCES players(id),
    level TEXT NOT NULL, position INTEGER NOT NULL, players_in_race INTEGER NOT NULL,
    total_time REAL, best_lap REAL, at INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS records (
    player_id INTEGER NOT NULL, level TEXT NOT NULL,
    best_lap REAL, best_total REAL,
    PRIMARY KEY (player_id, level)
);
CREATE INDEX IF NOT EXISTS rec_lap ON records(level, best_lap);
CREATE INDEX IF NOT EXISTS rec_total ON records(level, best_total);
";

enum Cmd {
    Register {
        name: String,
        country: [char; 2],
        hash: [u8; 32],
        reply: oneshot::Sender<Result<i64, NameError>>,
    },
    Update {
        player: i64,
        name: String,
        country: [char; 2],
        reply: oneshot::Sender<Result<(), NameError>>,
    },
    Auth {
        hash: [u8; 32],
        reply: oneshot::Sender<Option<(i64, String, [char; 2])>>,
    },
    Record {
        player: i64,
        level: String,
        pos: u32,
        of: u32,
        total: Option<f32>,
        best_lap: Option<f32>,
    },
    Board {
        level: String,
        kind: BoardKind,
        player: Option<i64>,
        reply: oneshot::Sender<ServerMsg>,
    },
}

#[derive(Clone)]
pub struct Db(mpsc::UnboundedSender<Cmd>);

impl Db {
    pub fn open(path: &str) -> anyhow::Result<Db> {
        let conn = Connection::open(path)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        conn.execute_batch(SCHEMA)?;
        let (tx, mut rx) = mpsc::unbounded_channel();
        std::thread::spawn(move || {
            while let Some(c) = rx.blocking_recv() {
                run(&conn, c);
            }
        });
        Ok(Db(tx))
    }

    pub async fn register(
        &self,
        name: String,
        country: [char; 2],
        hash: [u8; 32],
    ) -> Result<i64, NameError> {
        let (reply, rx) = oneshot::channel();
        let _ = self.0.send(Cmd::Register {
            name,
            country,
            hash,
            reply,
        });
        rx.await.unwrap_or(Err(NameError::Busy))
    }
    pub async fn update(
        &self,
        player: i64,
        name: String,
        country: [char; 2],
    ) -> Result<(), NameError> {
        let (reply, rx) = oneshot::channel();

        let _ = self.0.send(Cmd::Update {
            player,
            name,
            country,
            reply,
        });

        rx.await.unwrap_or(Err(NameError::Busy))
    }
    pub async fn auth(&self, hash: [u8; 32]) -> Option<(i64, String, [char; 2])> {
        let (reply, rx) = oneshot::channel();
        let _ = self.0.send(Cmd::Auth { hash, reply });
        rx.await.ok().flatten()
    }
    pub fn record(
        &self,
        player: i64,
        level: String,
        pos: u32,
        of: u32,
        total: Option<f32>,
        best_lap: Option<f32>,
    ) {
        let _ = self.0.send(Cmd::Record {
            player,
            level,
            pos,
            of,
            total,
            best_lap,
        });
    }
    pub async fn board(&self, level: String, kind: BoardKind, player: Option<i64>) -> ServerMsg {
        let (reply, rx) = oneshot::channel();
        let _ = self.0.send(Cmd::Board {
            level,
            kind,
            player,
            reply,
        });
        rx.await.unwrap_or(ServerMsg::Leaderboard {
            kind,
            rows: vec![],
            mine: None,
        })
    }
}

fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64)
}
fn upsert_min(col: &str) -> String {
    format!(
        "{col} = CASE WHEN excluded.{col} IS NULL THEN {col} WHEN {col} IS NULL THEN excluded.{col} ELSE MIN({col}, excluded.{col}) END"
    )
}

fn run(conn: &Connection, cmd: Cmd) {
    match cmd {
        Cmd::Register {
            name,
            country,
            hash,
            reply,
        } => {
            let r = conn.execute(
                "INSERT INTO players(name, country, token_hash, created_at) VALUES(?1, ?2, ?3, ?4)",
                params![
                    name,
                    country.iter().collect::<String>(),
                    hash.as_slice(),
                    now()
                ],
            );
            let _ = reply.send(match r {
                Ok(_) => Ok(conn.last_insert_rowid()),
                Err(rusqlite::Error::SqliteFailure(e, _))
                    if e.code == ErrorCode::ConstraintViolation =>
                {
                    Err(NameError::Taken)
                }
                Err(e) => {
                    eprintln!("db register: {e}");
                    Err(NameError::Busy)
                }
            });
        }
        Cmd::Update {
            player,
            name,
            country,
            reply,
        } => {
            let r = conn.execute(
                "UPDATE players SET name = ?1, country = ?2 WHERE id = ?3",
                params![name, country.iter().collect::<String>(), player],
            );

            let result = match r {
                Ok(1) => Ok(()),
                Ok(0) => Err(NameError::Busy),
                Ok(_) => Err(NameError::Busy),
                Err(rusqlite::Error::SqliteFailure(e, _))
                    if e.code == ErrorCode::ConstraintViolation =>
                {
                    Err(NameError::Taken)
                }
                Err(e) => {
                    eprintln!("db update profile: {e}");
                    Err(NameError::Busy)
                }
            };

            let _ = reply.send(result);
        }
        Cmd::Auth { hash, reply } => {
            let r = conn
                .query_row(
                    "SELECT id, name, country FROM players WHERE token_hash = ?1",
                    [hash.as_slice()],
                    |r| {
                        Ok((
                            r.get::<_, i64>(0)?,
                            r.get::<_, String>(1)?,
                            r.get::<_, String>(2)?
                                .chars()
                                .collect::<Vec<_>>()
                                .try_into()
                                .map_err(|_| rusqlite::Error::InvalidQuery)?,
                        ))
                    },
                )
                .optional();
            let _ = reply.send(r.unwrap_or(None));
        }
        Cmd::Record {
            player,
            level,
            pos,
            of,
            total,
            best_lap,
        } => {
            let r = (|| -> rusqlite::Result<()> {
                let tx = conn.unchecked_transaction()?;
                tx.execute(
                    "INSERT INTO results(player_id, level, position, players_in_race, total_time, best_lap, at)
                    VALUES(?1,?2,?3,?4,?5,?6,?7)",
                    params![player, level, pos, of, total, best_lap, now()],
                )?;
                tx.execute(
                    &format!(
                        "INSERT INTO records(player_id, level, best_lap, best_total) VALUES(?1,?2,?3,?4)
                         ON CONFLICT(player_id, level) DO UPDATE SET {}, {}",
                        upsert_min("best_lap"),
                        upsert_min("best_total")
                    ),
                    params![player, level, best_lap, total],
                )?;
                tx.commit()
            })();
            if let Err(e) = r {
                eprintln!("db record: {e}");
            }
        }
        Cmd::Board {
            level,
            kind,
            player,
            reply,
        } => {
            let msg = board(conn, level, kind, player).unwrap_or(ServerMsg::Leaderboard {
                kind,
                rows: vec![],
                mine: None,
            });
            let _ = reply.send(msg);
        }
    }
}

fn board(
    conn: &Connection,
    level: String,
    kind: BoardKind,
    player: Option<i64>,
) -> rusqlite::Result<ServerMsg> {
    let col = match kind {
        BoardKind::Lap => "best_lap",
        BoardKind::Total => "best_total",
    };
    let mut st = conn.prepare(&format!(
        "SELECT p.name, p.country, r.{col}
         FROM records r
         JOIN players p ON p.id = r.player_id
         WHERE r.level = ?1 AND r.{col} IS NOT NULL
         ORDER BY r.{col}
         LIMIT 10"
    ))?;
    let rows = st
        .query_map([&level], |r| {
            let country: String = r.get(1)?;
            let country: [char; 2] = country
                .chars()
                .collect::<Vec<_>>()
                .try_into()
                .map_err(|_| rusqlite::Error::InvalidQuery)?;

            Ok((r.get::<_, String>(0)?, r.get::<_, f64>(2)? as f32, country))
        })?
        .collect::<Result<Vec<_>, _>>()?;

    let mut mine = None;
    if let Some(id) = player {
        let v: Option<f64> = conn
            .query_row(
                &format!("SELECT {col} FROM records WHERE player_id = ?1 AND level = ?2"),
                params![id, level],
                |r| r.get(0),
            )
            .optional()?
            .flatten();
        if let Some(v) = v {
            let better: u32 = conn.query_row(
                &format!("SELECT COUNT(*) FROM records WHERE level = ?1 AND {col} < ?2"),
                params![level, v],
                |r| r.get(0),
            )?;
            mine = Some((better + 1, v as f32));
        }
    }
    Ok(ServerMsg::Leaderboard { kind, rows, mine })
}
