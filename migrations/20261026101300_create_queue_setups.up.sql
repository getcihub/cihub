-- The last setup read of a queue: what the rules of the SCM miss. Not a column
-- of `queues`: the lease of the queues skips a locked row.
CREATE TABLE queue_setups (
    org_id     text        NOT NULL REFERENCES orgs (id),
    queue_id   text        PRIMARY KEY,
    findings   jsonb       NOT NULL,
    checked_at timestamptz NOT NULL,
    FOREIGN KEY (org_id, queue_id) REFERENCES queues (org_id, id)
);

SELECT cihub_tenant_table('queue_setups');
