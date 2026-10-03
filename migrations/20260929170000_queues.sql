-- Queues and their leases (DECISIONS.md D54, D56). A queue is one target branch of
-- a repository, under a name unique in the repository, so that several queues
-- can target one branch (D135). It keeps the last valid configuration file read
-- on the head of its target (D136). A lease gives a queue to one scheduler
-- replica until it expires.

CREATE TYPE queue_state AS ENUM ('running', 'paused', 'draining', 'switching_modes');
CREATE TYPE queue_mode AS ENUM ('single', 'parallel');

CREATE TABLE queues (
    id            text        PRIMARY KEY,
    org_id        text        NOT NULL REFERENCES orgs (id),
    repo_id       text        NOT NULL,
    target_branch text        NOT NULL,
    name          text        NOT NULL,
    -- Projections of the log (D95), like the tables of the merge queue.
    state         queue_state NOT NULL DEFAULT 'running',
    mode          queue_mode  NOT NULL DEFAULT 'single',
    -- Something changed since the last reconciliation.
    dirty         boolean     NOT NULL DEFAULT false,
    -- The next periodic reconciliation, even without a change.
    next_tick_at  timestamptz,
    -- The text of the last valid file; NULL when the target has no file.
    config_text   text,
    -- The head of the target at the last read, valid or not.
    config_sha    text,
    -- Why the file read at config_sha is not valid; NULL when it is.
    config_error  text,
    created_at    timestamptz NOT NULL DEFAULT now(),
    UNIQUE (repo_id, name),
    UNIQUE (org_id, id),
    FOREIGN KEY (org_id, repo_id) REFERENCES repos (org_id, id)
);

CREATE INDEX queues_org_id ON queues (org_id);
CREATE INDEX queues_work ON queues (next_tick_at) WHERE dirty OR next_tick_at IS NOT NULL;

CREATE TABLE queue_leases (
    queue_id    text        PRIMARY KEY,
    org_id      text        NOT NULL,
    owner       text        NOT NULL,
    expires_at  timestamptz NOT NULL,
    FOREIGN KEY (org_id, queue_id) REFERENCES queues (org_id, id) ON DELETE CASCADE
);

CREATE INDEX queue_leases_owner ON queue_leases (owner);

SELECT cihub_tenant_table('queues');
SELECT cihub_tenant_table('queue_leases');
