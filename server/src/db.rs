use std::time::Duration;

use sqlx::{AssertSqlSafe, PgPool, postgres::PgPoolOptions};
use wrum_core::replay::{REPLAY_FORMAT_VERSION, REPLAY_SIM_VERSION};
use wrum_proto::{BoardKind, DeviceInfo, NameError, ServerMsg};

#[derive(Clone, Debug)]
pub struct Account {
    pub id: i64,
    pub name: String,
    pub country: [char; 2],
    pub email: Option<String>,
}

type Row = (i64, String, String, Option<String>);
const ACCOUNT_COLS: &str = "id, name, country, email";

pub enum EmailOutcome {
    Attached,
    LoggedInto(Account),
    NeedName,
}
fn account(r: Row) -> Account {
    Account {
        id: r.0,
        name: r.1,
        country: country_arr(&r.2),
        email: r.3,
    }
}

#[derive(Clone)]
pub struct Db(PgPool);

impl Db {
    pub async fn open(url: &str) -> anyhow::Result<Db> {
        let pool = PgPoolOptions::new()
            .max_connections(8)
            .acquire_timeout(Duration::from_secs(3))
            .connect(url)
            .await?;
        sqlx::migrate!("./migrations").run(&pool).await?;
        Ok(Db(pool))
    }

    pub async fn register(
        &self,
        name: String,
        country: [char; 2],
        email: Option<String>,
        hash: [u8; 32],
    ) -> Result<i64, NameError> {
        let country: String = country.iter().collect();
        let res: sqlx::Result<i64> = async {
                    let mut tx = self.0.begin().await?;
                    let id: i64 = sqlx::query_scalar(
                        "INSERT INTO players(name, country, email, created_at) VALUES($1, $2, $3, $4) RETURNING id",
                    )
                    .bind(name).bind(country).bind(email).bind(now())
                    .fetch_one(&mut *tx).await?;
                    insert_device(&mut tx, id, hash).await?;
                    tx.commit().await?;
                    Ok(id)
                }.await;
        match res {
            Ok(id) => Ok(id),
            Err(sqlx::Error::Database(e)) if e.is_unique_violation() => {
                if e.constraint() == Some("players_email") {
                    Err(NameError::Busy)
                } else {
                    Err(NameError::Taken)
                }
            }
            Err(e) => {
                eprintln!("db register: {e}");
                Err(NameError::Busy)
            }
        }
    }
    pub async fn update(
        &self,
        player: i64,
        name: String,
        country: [char; 2],
    ) -> Result<(), NameError> {
        let country: String = country.iter().collect();

        match sqlx::query("UPDATE players SET name = $1, country = $2 WHERE id = $3")
            .bind(name)
            .bind(country)
            .bind(player)
            .execute(&self.0)
            .await
        {
            Ok(result) if result.rows_affected() == 1 => Ok(()),
            Ok(_) => Err(NameError::Busy),
            Err(sqlx::Error::Database(e)) if e.is_unique_violation() => Err(NameError::Taken),
            Err(e) => {
                eprintln!("db update: {e}");
                Err(NameError::Busy)
            }
        }
    }
    pub async fn auth(&self, hash: [u8; 32]) -> Option<Account> {
        sqlx::query_as::<_, Row>(
            "UPDATE devices d SET last_seen = $2 FROM players p
                     WHERE d.token_hash = $1 AND p.id = d.player_id
                     RETURNING p.id, p.name, p.country, p.email",
        )
        .bind(&hash[..])
        .bind(now())
        .fetch_optional(&self.0)
        .await
        .map_err(|e| eprintln!("db auth: {e}"))
        .ok()
        .flatten()
        .map(account)
    }
    pub async fn record(
        &self,
        player: i64,
        level: String,
        pos: u32,
        of: u32,
        total: Option<f32>,
        best_lap: Option<f32>,
        replay: Option<(Vec<u8>, u8)>,
    ) -> sqlx::Result<()> {
        let mut tx = self.0.begin().await?;
        sqlx::query(
            "INSERT INTO results(player_id, level, position, players_in_race, total_time, best_lap, at)
             VALUES($1, $2, $3, $4, $5, $6, $7)",
        )
        .bind(player)
        .bind(&level)
        .bind(pos as i32)
        .bind(of as i32)
        .bind(total)
        .bind(best_lap)
        .bind(now())
        .execute(&mut *tx)
        .await?;

        sqlx::query(
            "INSERT INTO records(player_id, level, best_lap, best_total) VALUES($1, $2, NULL, NULL)
             ON CONFLICT (player_id, level) DO NOTHING",
        )
        .bind(player)
        .bind(&level)
        .execute(&mut *tx)
        .await?;

        let (old_total,): (Option<f32>,) = sqlx::query_as(
            "SELECT best_total
            FROM records
            WHERE player_id = $1 AND level = $2
            FOR UPDATE",
        )
        .bind(player)
        .bind(&level)
        .fetch_one(&mut *tx)
        .await?;

        let improved_total = total.is_some_and(|t| old_total.is_none_or(|old| t < old));

        let mut replay_id: Option<i64> = None;
        let mut replay_slot: Option<i16> = None;

        if improved_total {
            if let Some((bytes, slot)) = replay {
                let id: i64 = sqlx::query_scalar(
                    "INSERT INTO replays (
                        level,
                        format_version,
                        sim_version,
                        data,
                        created_at
                    )
                    VALUES ($1, $2, $3, $4, $5)
                    RETURNING id",
                )
                .bind(&level)
                .bind(REPLAY_FORMAT_VERSION as i16)
                .bind(REPLAY_SIM_VERSION as i16)
                .bind(bytes)
                .bind(now())
                .fetch_one(&mut *tx)
                .await?;

                replay_id = Some(id);
                replay_slot = Some(slot as i16);
            }
        }

        sqlx::query(
            r#"
                UPDATE records
                SET
                    best_lap = CASE
                        WHEN $3::REAL IS NULL THEN best_lap
                        WHEN best_lap IS NULL THEN $3
                        ELSE LEAST(best_lap, $3)
                    END,
                    best_total = CASE
                        WHEN $4::REAL IS NOT NULL
                         AND (best_total IS NULL OR $4 < best_total)
                        THEN $4
                        ELSE best_total
                    END,
                    best_total_replay = CASE
                        WHEN $6 THEN $5
                        ELSE best_total_replay
                    END,
                    best_total_slot = CASE
                        WHEN $6 THEN $7
                        ELSE best_total_slot
                    END
                WHERE player_id = $1 AND level = $2
                "#,
        )
        .bind(player)
        .bind(&level)
        .bind(best_lap)
        .bind(total)
        .bind(replay_id)
        .bind(improved_total)
        .bind(replay_slot)
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;
        Ok(())
    }
    pub async fn board(&self, level: String, kind: BoardKind, player: Option<i64>) -> ServerMsg {
        board(&self.0, &level, kind, player)
            .await
            .unwrap_or_else(|e| {
                eprintln!("db board: {e}");
                ServerMsg::Leaderboard {
                    kind,
                    rows: vec![],
                    mine: None,
                }
            })
    }
    pub async fn replay(&self, id: i64) -> sqlx::Result<Option<(Vec<u8>, u8)>> {
        eprintln!("replay: {id}");
        let row: Option<(Vec<u8>, i16)> = sqlx::query_as(
            "SELECT r.data, rec.best_total_slot
            FROM replays r
            JOIN records rec ON rec.best_total_replay = r.id
            WHERE r.id = $1",
        )
        .bind(id)
        .fetch_optional(&self.0)
        .await?;

        Ok(row.and_then(|(data, slot)| u8::try_from(slot).ok().map(|slot| (data, slot))))
    }

    pub async fn email_verified(
        &self,
        email: &str,
        current: Option<i64>,
        hash: [u8; 32],
    ) -> sqlx::Result<EmailOutcome> {
        let mut tx = self.0.begin().await?;
        let owner: Option<Row> = sqlx::query_as(AssertSqlSafe(format!(
            "SELECT {ACCOUNT_COLS} FROM players WHERE email = $1"
        )))
        .bind(email)
        .fetch_optional(&mut *tx)
        .await?;
        let out = match (owner, current) {
            (Some(o), Some(c)) if o.0 == c => EmailOutcome::Attached,
            (Some(o), _) => {
                insert_device(&mut tx, o.0, hash).await?;
                EmailOutcome::LoggedInto(account(o))
            }
            (None, Some(c)) => {
                sqlx::query("UPDATE players SET email = $1 WHERE id = $2")
                    .bind(email)
                    .bind(c)
                    .execute(&mut *tx)
                    .await?;
                EmailOutcome::Attached
            }
            (None, None) => EmailOutcome::NeedName,
        };
        tx.commit().await?;
        Ok(out)
    }
    pub async fn add_device(&self, id: i64, hash: [u8; 32]) -> Option<Account> {
        let r: sqlx::Result<Option<Row>> = async {
            let mut tx = self.0.begin().await?;
            let row: Option<Row> = sqlx::query_as(AssertSqlSafe(format!(
                "SELECT {ACCOUNT_COLS} FROM players WHERE id = $1"
            )))
            .bind(id)
            .fetch_optional(&mut *tx)
            .await?;
            if row.is_some() {
                insert_device(&mut tx, id, hash).await?;
            }
            tx.commit().await?;
            Ok(row)
        }
        .await;
        r.map_err(|e| eprintln!("db add_device: {e}"))
            .ok()
            .flatten()
            .map(account)
    }
    pub async fn devices(&self, player: i64, current: Option<[u8; 32]>) -> Vec<DeviceInfo> {
        let cur = current.map(|h| hex::encode(&h[..6]));
        sqlx::query_as::<_, (String, i64, i64)>(
            "SELECT encode(substring(token_hash from 1 for 6), 'hex'), created_at, last_seen
                 FROM devices WHERE player_id = $1 ORDER BY last_seen DESC",
        )
        .bind(player)
        .fetch_all(&self.0)
        .await
        .map_err(|e| eprintln!("db devices: {e}"))
        .unwrap_or_default()
        .into_iter()
        .map(|(id, created_at, last_seen)| DeviceInfo {
            current: cur.as_deref() == Some(&id),
            id,
            created_at,
            last_seen,
        })
        .collect()
    }
    pub async fn revoke_device(&self, player: i64, id: &str) {
        let _ = sqlx::query(
                "DELETE FROM devices WHERE player_id = $1 AND encode(substring(token_hash from 1 for 6), 'hex') = $2",
            )
            .bind(player).bind(id).execute(&self.0).await
            .map_err(|e| eprintln!("db revoke: {e}"));
    }
    pub async fn logout_device(&self, hash: [u8; 32]) {
        let _ = sqlx::query("DELETE FROM devices WHERE token_hash = $1")
            .bind(&hash[..])
            .execute(&self.0)
            .await;
    }
    pub async fn logout_all(&self, player: i64) {
        let _ = sqlx::query("DELETE FROM devices WHERE player_id = $1")
            .bind(player)
            .execute(&self.0)
            .await;
    }
    pub async fn delete_account(&self, player: i64) {
        let _ = sqlx::query("DELETE FROM players WHERE id = $1")
            .bind(player)
            .execute(&self.0)
            .await
            .map_err(|e| eprintln!("db delete_account: {e}"));
    }
}

fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64)
}
fn country_arr(s: &str) -> [char; 2] {
    let mut c = s.chars();
    [c.next().unwrap_or('?'), c.next().unwrap_or('?')]
}

async fn board(
    pool: &PgPool,
    level: &str,
    kind: BoardKind,
    player: Option<i64>,
) -> sqlx::Result<ServerMsg> {
    let rows: Vec<(String, f32, String, Option<i64>)> = match kind {
        BoardKind::Lap => {
            sqlx::query_as(
                "SELECT p.name, r.best_lap, p.country, NULL::BIGINT
                 FROM records r
                 JOIN players p ON p.id = r.player_id
                 WHERE r.level = $1 AND r.best_lap IS NOT NULL
                 ORDER BY r.best_lap
                 LIMIT 10",
            )
            .bind(level)
            .fetch_all(pool)
            .await?
        }
        BoardKind::Total => {
            sqlx::query_as(
                "SELECT p.name, r.best_total, p.country, r.best_total_replay
                 FROM records r
                 JOIN players p ON p.id = r.player_id
                 WHERE r.level = $1 AND r.best_total IS NOT NULL
                 ORDER BY r.best_total
                 LIMIT 10",
            )
            .bind(level)
            .fetch_all(pool)
            .await?
        }
    };
    let rows = rows
        .into_iter()
        .map(|(n, t, c, r)| (n, t, country_arr(&c), r))
        .collect();

    let mut mine = None;
    if let Some(id) = player {
        let v: Option<f32> = match kind {
            BoardKind::Lap => sqlx::query_scalar(
                "SELECT best_lap FROM records
                     WHERE player_id = $1 AND level = $2",
            )
            .bind(id)
            .bind(level)
            .fetch_optional(pool)
            .await?
            .flatten(),
            BoardKind::Total => sqlx::query_scalar(
                "SELECT best_total FROM records
                     WHERE player_id = $1 AND level = $2",
            )
            .bind(id)
            .bind(level)
            .fetch_optional(pool)
            .await?
            .flatten(),
        };

        if let Some(v) = v {
            let better: i64 = match kind {
                BoardKind::Lap => {
                    sqlx::query_scalar(
                        "SELECT COUNT(*) FROM records
                         WHERE level = $1 AND best_lap < $2",
                    )
                    .bind(level)
                    .bind(v)
                    .fetch_one(pool)
                    .await?
                }
                BoardKind::Total => {
                    sqlx::query_scalar(
                        "SELECT COUNT(*) FROM records
                         WHERE level = $1 AND best_total < $2",
                    )
                    .bind(level)
                    .bind(v)
                    .fetch_one(pool)
                    .await?
                }
            };
            mine = Some((better as u32 + 1, v));
        }
    }
    Ok(ServerMsg::Leaderboard { kind, rows, mine })
}

async fn insert_device(
    c: &mut sqlx::PgConnection,
    player: i64,
    hash: [u8; 32],
) -> sqlx::Result<()> {
    sqlx::query(
        "INSERT INTO devices(token_hash, player_id, created_at, last_seen) VALUES($1, $2, $3, $3)",
    )
    .bind(&hash[..])
    .bind(player)
    .bind(now())
    .execute(c)
    .await?;
    Ok(())
}
