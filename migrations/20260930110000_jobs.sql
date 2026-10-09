CREATE TYPE job_class AS ENUM ('interactive', 'merge', 'speculate', 'repair');
CREATE TYPE job_state AS ENUM ('queued', 'running', 'done', 'failed', 'dead');

CREATE TABLE jobs (
    idempotency_key text        PRIMARY KEY,
    type            text        NOT NULL,
    class           job_class   NOT NULL,
    org_id          text        REFERENCES orgs (id),
    connection_id   text,
    queue_id        text,
    payload         jsonb       NOT NULL,
    state           job_state   NOT NULL DEFAULT 'queued',
    attempts        integer     NOT NULL DEFAULT 0,
    run_after       timestamptz NOT NULL,
    locked_by       text,
    locked_until    timestamptz,
    last_error      text,
    created_at      timestamptz NOT NULL,
    finished_at     timestamptz,
    FOREIGN KEY (org_id, connection_id) REFERENCES scm_connections (org_id, id),
    FOREIGN KEY (org_id, queue_id) REFERENCES queues (org_id, id),
    CHECK (org_id IS NOT NULL OR (connection_id IS NULL AND queue_id IS NULL)),
    CHECK ((state = 'running') = (locked_until IS NOT NULL)),
    CHECK ((state IN ('done', 'failed', 'dead')) = (finished_at IS NOT NULL))
);

CREATE INDEX jobs_ready ON jobs (class, run_after) WHERE state = 'queued';

CREATE INDEX jobs_running ON jobs (connection_id, locked_until) WHERE state = 'running';

SELECT cihub_tenant_table('jobs');
