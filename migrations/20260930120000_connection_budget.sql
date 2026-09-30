-- The budget and the mode of a connection (AGENTS.md D70, D77). Each process
-- keeps the rate limit headers of the SCM in memory and writes them here at
-- most every 5 s; a pause after a 429 or a change of mode is written at once.
-- The claim of the jobs skips a connection until `paused_until`.

CREATE TYPE connection_mode AS ENUM ('normal', 'throttled', 'scm_unavailable', 'paused');

ALTER TABLE scm_connections
    ADD COLUMN mode                 connection_mode NOT NULL DEFAULT 'normal',
    -- Why the connection left `normal`, for the operator.
    ADD COLUMN mode_reason          text,
    ADD COLUMN rate_remaining       integer,
    ADD COLUMN rate_limit           integer,
    ADD COLUMN rate_reset_at        timestamptz,
    ADD COLUMN paused_until         timestamptz,
    ADD COLUMN consecutive_failures integer         NOT NULL DEFAULT 0;
