CREATE TABLE devices (
  token_hash BYTEA PRIMARY KEY,
  player_id BIGINT NOT NULL REFERENCES players(id) ON DELETE CASCADE,
  created_at BIGINT NOT NULL,
  last_seen BIGINT NOT NULL
);
CREATE INDEX devices_player ON devices(player_id);

INSERT INTO devices(token_hash, player_id, created_at, last_seen)
SELECT token_hash, id, created_at, created_at FROM players;

ALTER TABLE players DROP COLUMN token_hash;

ALTER TABLE players ADD COLUMN email TEXT;
CREATE UNIQUE INDEX players_email ON players(email) WHERE email IS NOT NULL;
