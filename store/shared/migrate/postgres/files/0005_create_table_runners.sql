-- name: create-table-runners

CREATE TABLE IF NOT EXISTS runners (
  runner_name            VARCHAR(255) PRIMARY KEY,
  runner_id              BIGINT,
  runner_installation_id BIGINT,
  runner_owner           VARCHAR(255),
  runner_status          VARCHAR(50),
  runner_machine         VARCHAR(255),
  runner_arch            VARCHAR(50),
  runner_cpu             BIGINT,
  runner_ram             BIGINT,
  runner_image           VARCHAR(255),
  runner_group_id        BIGINT,
  runner_labels          TEXT,
  runner_cancelled       BIGINT,
  runner_created         BIGINT,
  runner_accepted        BIGINT,
  runner_started         BIGINT,
  runner_stopped         BIGINT,
  runner_updated         BIGINT,
  runner_token           TEXT
);

-- name: create-index-runners-status

CREATE INDEX IF NOT EXISTS ix_runner_status ON runners (runner_status);

-- name: create-index-runners-machine

CREATE INDEX IF NOT EXISTS ix_runner_machine ON runners (runner_machine);

-- name: create-index-runners-created

CREATE INDEX IF NOT EXISTS ix_runner_created ON runners (runner_created);
