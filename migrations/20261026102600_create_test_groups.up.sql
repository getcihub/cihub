CREATE TYPE group_state AS ENUM (
    'testing', 'pending_failure', 'tests_passed', 'merged', 'failed', 'bisected', 'dissolved'
);

CREATE TABLE test_groups (
    id              text        PRIMARY KEY,
    org_id          text        NOT NULL REFERENCES orgs (id),
    queue_id        text        NOT NULL,
    item_ids        text[]      NOT NULL,
    predecessor_ids text[]      NOT NULL,
    state           group_state NOT NULL,
    UNIQUE (org_id, id),
    FOREIGN KEY (org_id, queue_id) REFERENCES queues (org_id, id)
);

CREATE INDEX test_groups_queue_id ON test_groups (queue_id);
-- The live groups of a queue: the graph.
CREATE INDEX test_groups_live ON test_groups (queue_id)
    WHERE state NOT IN ('merged', 'failed', 'bisected', 'dissolved');

SELECT cihub_tenant_table('test_groups');
