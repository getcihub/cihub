CREATE TABLE webhook_inbox (
    scm_instance_id text        NOT NULL REFERENCES scm_instances (id),
    delivery_id     text        NOT NULL,
    org_id          text        REFERENCES orgs (id),
    connection_id   text,
    queue_id        text,
    event           text        NOT NULL,
    payload         jsonb       NOT NULL,
    received_at     timestamptz NOT NULL,
    routed_at       timestamptz,
    processed_at    timestamptz,
    error           text,
    PRIMARY KEY (scm_instance_id, delivery_id),
    FOREIGN KEY (org_id, scm_instance_id, connection_id)
        REFERENCES scm_connections (org_id, scm_instance_id, id),
    FOREIGN KEY (org_id, queue_id) REFERENCES queues (org_id, id),
    CHECK (org_id IS NOT NULL OR (connection_id IS NULL AND queue_id IS NULL))
);

CREATE INDEX webhook_inbox_pending ON webhook_inbox (queue_id, received_at)
    WHERE processed_at IS NULL;

CREATE INDEX webhook_inbox_unrouted ON webhook_inbox (received_at)
    WHERE routed_at IS NULL;

SELECT cihub_tenant_table('webhook_inbox');
