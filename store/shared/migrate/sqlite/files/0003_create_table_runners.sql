-- name: create-table-runners

CREATE TABLE IF NOT EXISTS runners (
  runner_name            TEXT PRIMARY KEY COLLATE NOCASE,
  runner_id              INTEGER,
  runner_installation_id INTEGER,
  runner_owner           TEXT,
  runner_status          TEXT,
  runner_assigned_to     INTEGER,
  runner_cancelled       BOOLEAN,
  runner_completed       INTEGER,
  runner_created         INTEGER,
  runner_started         INTEGER,
  runner_stopped         INTEGER,
  runner_updated         INTEGER,
  runner_timeout         INTEGER,
  runner_token           TEXT
);

CREATE INDEX IF NOT EXISTS ix_runner_status ON runners (runner_status);
CREATE INDEX IF NOT EXISTS ix_runner_assigned_to ON runners (runner_assigned_to);
CREATE INDEX IF NOT EXISTS ix_runner_created ON runners (runner_created);
