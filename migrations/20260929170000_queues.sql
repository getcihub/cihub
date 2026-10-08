CREATE TYPE queue_state AS ENUM ('running', 'paused', 'draining', 'switching_modes');
CREATE TYPE queue_mode AS ENUM ('single', 'parallel');

CREATE TABLE queues (
    id            text        PRIMARY KEY,
    org_id        text        NOT NULL REFERENCES orgs (id),
    repo_id       text        NOT NULL,
    target_branch text        NOT NULL,
    name          text        NOT NULL,
    state         queue_state NOT NULL DEFAULT 'running',
    mode          queue_mode  NOT NULL DEFAULT 'single',
    dirty         boolean     NOT NULL DEFAULT false,
    next_tick_at  timestamptz,
    lease_owner   text,
    lease_expires_at timestamptz,
    config_text   text,
    config_sha    text,
    config_error  text,
    created_at    timestamptz NOT NULL DEFAULT now(),
    UNIQUE (repo_id, name),
    UNIQUE (org_id, id),
    FOREIGN KEY (org_id, repo_id) REFERENCES repos (org_id, id)
);

CREATE INDEX queues_org_id ON queues (org_id);
CREATE INDEX queues_work ON queues (next_tick_at) WHERE dirty OR next_tick_at IS NOT NULL;

SELECT cihub_tenant_table('queues');
