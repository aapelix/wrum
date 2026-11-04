CREATE TABLE replays (
    id BIGSERIAL PRIMARY KEY,
    level TEXT NOT NULL,
    format_version SMALLINT NOT NULL,
    sim_version SMALLINT NOT NULL,
    data BYTEA NOT NULL,
    created_at BIGINT NOT NULL
);

ALTER TABLE records
    ADD COLUMN best_total_replay BIGINT
        REFERENCES replays(id) ON DELETE SET NULL;

ALTER TABLE records
    ADD COLUMN best_total_slot SMALLINT;
