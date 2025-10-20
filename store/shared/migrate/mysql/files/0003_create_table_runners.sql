-- name: create-table-runners

CREATE TABLE IF NOT EXISTS runners (
  runner_name            VARCHAR(255) PRIMARY KEY,
  runner_id              BIGINT,
  runner_installation_id BIGINT,
  runner_owner           VARCHAR(255),
  runner_repo            VARCHAR(255),
  runner_status          VARCHAR(50),
  runner_assigned_to     BIGINT,
  runner_cancelled       BOOLEAN,
  runner_completed       BIGINT,
  runner_created         BIGINT,
  runner_started         BIGINT,
  runner_stopped         BIGINT,
  runner_updated         BIGINT,
  runner_timeout         BIGINT,
  runner_token           TEXT
);

CREATE INDEX ix_runner_status ON runners (runner_status);
CREATE INDEX ix_runner_assigned_to ON runners (runner_assigned_to);
CREATE INDEX ix_runner_created ON runners (runner_created);
