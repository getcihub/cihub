-- How far each subscription read the log of a queue: `events` is the outbox.
CREATE TABLE log_cursors (
    org_id       text   NOT NULL REFERENCES orgs (id),
    queue_id     text   NOT NULL,
    subscription text   NOT NULL,
    seq          bigint NOT NULL,
    PRIMARY KEY (queue_id, subscription),
    FOREIGN KEY (org_id, queue_id) REFERENCES queues (org_id, id)
);

SELECT cihub_tenant_table('log_cursors');
