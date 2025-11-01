-- name: create-table-jobs

CREATE TABLE IF NOT EXISTS jobs (
  job_id              INTEGER PRIMARY KEY,
  job_run_id          INTEGER,
  job_installation_id INTEGER,
  job_owner           TEXT,
  job_repo            TEXT,
  job_workflow        TEXT,
  job_name            TEXT,
  job_branch          TEXT,
  job_sha             TEXT,
  job_status          TEXT,
  job_conclusion      TEXT,
  job_labels          TEXT,
  job_runner_id       INTEGER,
  job_runner_name     TEXT,
  job_url             TEXT,
  job_author_login    TEXT,
  job_author_avatar   TEXT,
  job_queued          INTEGER,
  job_started         INTEGER,
  job_completed       INTEGER,
  job_created         INTEGER,
  job_updated         INTEGER,
  job_version         INTEGER
);

CREATE INDEX IF NOT EXISTS ix_job_run_id ON jobs (job_run_id);
CREATE INDEX IF NOT EXISTS ix_job_status ON jobs (job_status);
CREATE INDEX IF NOT EXISTS ix_job_runner_id ON jobs (job_runner_id);
CREATE INDEX IF NOT EXISTS ix_job_created ON jobs (job_created);
