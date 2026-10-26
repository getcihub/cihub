CREATE TABLE webhook_inbox (
    delivery_id   text        PRIMARY KEY,
    org_id        text        REFERENCES orgs (id),
    connection_id text,
    queue_id      text,
    event         text        NOT NULL,
    payload       jsonb       NOT NULL,
    received_at   timestamptz NOT NULL,
    attempts      integer     NOT NULL DEFAULT 0,
    set_aside_at  timestamptz,
    FOREIGN KEY (org_id, connection_id) REFERENCES scm_connections (org_id, id),
    FOREIGN KEY (org_id, queue_id) REFERENCES queues (org_id, id),
    CHECK (org_id IS NOT NULL OR (connection_id IS NULL AND queue_id IS NULL))
);

CREATE INDEX webhook_inbox_pending ON webhook_inbox (queue_id, received_at)
    WHERE set_aside_at IS NULL;

CREATE INDEX webhook_inbox_unrouted ON webhook_inbox (received_at)
    WHERE connection_id IS NULL;

SELECT cihub_tenant_table('webhook_inbox');
