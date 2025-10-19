-- name: create-table-jobs

CREATE TABLE IF NOT EXISTS jobs (
  job_id              BIGINT PRIMARY KEY,
  job_run_id          BIGINT,
  job_installation_id BIGINT,
  job_owner           VARCHAR(255),
  job_repo            VARCHAR(255),
  job_workflow        VARCHAR(500),
  job_name            VARCHAR(500),
  job_branch          VARCHAR(255),
  job_sha             VARCHAR(255),
  job_status          VARCHAR(50),
  job_conclusion      VARCHAR(50),
  job_labels          TEXT,
  job_runner_id       BIGINT,
  job_runner_name     VARCHAR(255),
  job_machine         VARCHAR(255),
  job_url             VARCHAR(1000),
  job_accepted        BIGINT,
  job_queued          BIGINT,
  job_started         BIGINT,
  job_completed       BIGINT,
  job_created         BIGINT,
  job_updated         BIGINT,
  job_version         BIGINT
);

CREATE INDEX IF NOT EXISTS ix_job_run_id ON jobs (job_run_id);
CREATE INDEX IF NOT EXISTS ix_job_status ON jobs (job_status);
CREATE INDEX IF NOT EXISTS ix_job_runner_id ON jobs (job_runner_id);
CREATE INDEX IF NOT EXISTS ix_job_machine ON jobs (job_machine);
CREATE INDEX IF NOT EXISTS ix_job_created ON jobs (job_created);
