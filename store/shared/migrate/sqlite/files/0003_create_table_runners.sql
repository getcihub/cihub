-- name: create-table-runners

CREATE TABLE IF NOT EXISTS runners (
  runner_name            TEXT PRIMARY KEY COLLATE NOCASE,
  runner_id              INTEGER,
  runner_installation_id INTEGER,
  runner_owner           TEXT,
  runner_status          TEXT,
  runner_machine         TEXT,
  runner_arch            TEXT,
  runner_cpu             INTEGER,
  runner_ram             INTEGER,
  runner_image           TEXT,
  runner_group_id        INTEGER,
  runner_labels          TEXT,
  runner_cancelled       INTEGER,
  runner_created         INTEGER,
  runner_accepted        INTEGER,
  runner_started         INTEGER,
  runner_stopped         INTEGER,
  runner_updated         INTEGER,
  runner_token           TEXT
);

CREATE INDEX IF NOT EXISTS ix_runner_status ON runners (runner_status);
CREATE INDEX IF NOT EXISTS ix_runner_created ON runners (runner_created);
CREATE INDEX IF NOT EXISTS ix_runner_machine ON runners (runner_machine);
