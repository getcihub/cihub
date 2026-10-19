-- The conditions status of each open PR on a branch with a queue, submitted
-- or not: the job that reads the PR is its only writer.
CREATE TABLE pr_conditions (
    org_id         text        NOT NULL REFERENCES orgs (id),
    queue_id       text        NOT NULL,
    pr_number      bigint      NOT NULL,
    head_sha       text        NOT NULL,
    desired        jsonb       NOT NULL,
    desired_hash   text        NOT NULL,
    published_hash text,
    -- An older read never replaces a newer one.
    observed_at    timestamptz NOT NULL,
    PRIMARY KEY (queue_id, pr_number),
    FOREIGN KEY (org_id, queue_id) REFERENCES queues (org_id, id)
);

-- A check on the head of a PR finds its rows.
CREATE INDEX pr_conditions_head ON pr_conditions (org_id, head_sha);

SELECT cihub_tenant_table('pr_conditions');
