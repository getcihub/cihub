-- Queues and their leases (AGENTS.md D54, D56). A queue is one target branch of
-- a repository; lot 5 adds its state, mode and settings. A lease gives a queue
-- to one scheduler replica until it expires.

-- A queue and its repository belong to the same organization.
ALTER TABLE repos ADD UNIQUE (org_id, id);

CREATE TABLE queues (
    id            text        PRIMARY KEY,
    org_id        text        NOT NULL REFERENCES orgs (id),
    repo_id       text        NOT NULL,
    target_branch text        NOT NULL,
    -- Something changed since the last reconciliation.
    dirty         boolean     NOT NULL DEFAULT false,
    -- The next periodic reconciliation, even without a change.
    next_tick_at  timestamptz,
    created_at    timestamptz NOT NULL DEFAULT now(),
    UNIQUE (repo_id, target_branch),
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

ALTER TABLE events ADD FOREIGN KEY (org_id, queue_id) REFERENCES queues (org_id, id);

ALTER TABLE queues ENABLE ROW LEVEL SECURITY;
ALTER TABLE queues FORCE ROW LEVEL SECURITY;
CREATE POLICY tenant ON queues
    USING (cihub_system() OR org_id = cihub_org_id())
    WITH CHECK (cihub_system() OR org_id = cihub_org_id());

ALTER TABLE queue_leases ENABLE ROW LEVEL SECURITY;
ALTER TABLE queue_leases FORCE ROW LEVEL SECURITY;
CREATE POLICY tenant ON queue_leases
    USING (cihub_system() OR org_id = cihub_org_id())
    WITH CHECK (cihub_system() OR org_id = cihub_org_id());
