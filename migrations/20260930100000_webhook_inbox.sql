-- The inbox of the webhook deliveries (DECISIONS.md D58). The ingestion stores a
-- delivery before any decision; a copy of a delivery changes nothing. A
-- delivery identifier comes from the SCM, so it is unique within its instance
-- only (D46).
--
-- The ingestion fills what it knows: the organization, the connection and the
-- queue stay empty for a delivery it cannot route yet (an installation of an
-- unknown account), which only the system context sees.

CREATE TABLE webhook_inbox (
    scm_instance_id text        NOT NULL REFERENCES scm_instances (id),
    delivery_id     text        NOT NULL,
    org_id          text        REFERENCES orgs (id),
    connection_id   text,
    queue_id        text,
    event           text        NOT NULL,
    action          text,
    payload         jsonb       NOT NULL,
    received_at     timestamptz NOT NULL,
    processed_at    timestamptz,
    -- Why the processing failed, for the operator.
    error           text,
    PRIMARY KEY (scm_instance_id, delivery_id),
    FOREIGN KEY (org_id, scm_instance_id, connection_id)
        REFERENCES scm_connections (org_id, scm_instance_id, id),
    FOREIGN KEY (org_id, queue_id) REFERENCES queues (org_id, id),
    CHECK (org_id IS NOT NULL OR (connection_id IS NULL AND queue_id IS NULL))
);

CREATE INDEX webhook_inbox_pending ON webhook_inbox (queue_id, received_at)
    WHERE processed_at IS NULL;

ALTER TABLE webhook_inbox ENABLE ROW LEVEL SECURITY;
ALTER TABLE webhook_inbox FORCE ROW LEVEL SECURITY;
CREATE POLICY tenant ON webhook_inbox
    USING (cihub_system() OR org_id = cihub_org_id())
    WITH CHECK (cihub_system() OR org_id = cihub_org_id());
