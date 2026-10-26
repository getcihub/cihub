-- The hourly aggregate of each queue, which the Health reads: its cost
-- follows the length of a period, not the number of PRs. The notifier adds
-- to it the facts that it writes, in their transaction. `hour` is the UTC
-- hour of the end of the item or of the run.
CREATE TABLE an_hourly_conclusions (
    org_id     text        NOT NULL REFERENCES orgs (id),
    queue_id   text        NOT NULL,
    hour       timestamptz NOT NULL,
    conclusion item_state  NOT NULL CHECK (conclusion IN ('merged', 'failed', 'cancelled')),
    reason     reason      NOT NULL,
    count      bigint      NOT NULL,
    PRIMARY KEY (queue_id, hour, conclusion, reason),
    FOREIGN KEY (org_id, queue_id) REFERENCES queues (org_id, id)
);

SELECT cihub_tenant_table('an_hourly_conclusions');
