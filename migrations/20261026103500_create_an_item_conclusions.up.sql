-- The facts are derived from the log only, so that a replay writes them again.
CREATE TABLE an_item_conclusions (
    item_id      text        PRIMARY KEY,
    org_id       text        NOT NULL REFERENCES orgs (id),
    queue_id     text        NOT NULL,
    conclusion   item_state  NOT NULL CHECK (conclusion IN ('merged', 'failed', 'cancelled')),
    reason       reason      NOT NULL,
    submitted_at timestamptz NOT NULL,
    concluded_at timestamptz NOT NULL,
    FOREIGN KEY (org_id, queue_id) REFERENCES queues (org_id, id)
);

CREATE INDEX an_item_conclusions_queue ON an_item_conclusions (queue_id, concluded_at);

SELECT cihub_tenant_table('an_item_conclusions');
