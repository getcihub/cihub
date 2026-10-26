CREATE TABLE an_test_run_results (
    run_id     text        PRIMARY KEY,
    org_id     text        NOT NULL REFERENCES orgs (id),
    queue_id   text        NOT NULL,
    group_id   text        NOT NULL,
    attempt    integer     NOT NULL,
    status     run_status  NOT NULL CHECK (status NOT IN ('assembling', 'in_progress')),
    reason     reason      NOT NULL,
    started_at timestamptz NOT NULL,
    ended_at   timestamptz NOT NULL,
    FOREIGN KEY (org_id, queue_id) REFERENCES queues (org_id, id)
);

CREATE INDEX an_test_run_results_queue ON an_test_run_results (queue_id, ended_at);
-- The attempt before a run of the same group: a check that passed on a retry.
CREATE INDEX an_test_run_results_group ON an_test_run_results (group_id, attempt);

SELECT cihub_tenant_table('an_test_run_results');
