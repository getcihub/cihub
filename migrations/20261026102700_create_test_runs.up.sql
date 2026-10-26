CREATE TYPE required_statuses_source AS ENUM ('repo_config', 'branch_protection');

CREATE TABLE test_runs (
    id                       text                     PRIMARY KEY,
    org_id                   text                     NOT NULL REFERENCES orgs (id),
    queue_id                 text                     NOT NULL,
    group_id                 text                     NOT NULL,
    attempt                  integer                  NOT NULL,
    test_branch              text                     NOT NULL,
    test_sha                 text,
    draft_pr_number          bigint,
    status                   run_status               NOT NULL,
    status_reason            reason                   NOT NULL,
    required_statuses        text[]                   NOT NULL,
    required_statuses_source required_statuses_source,
    started_at               timestamptz              NOT NULL,
    ended_at                 timestamptz,
    failure_causing_statuses text[]                   NOT NULL,
    conflict_culprit         jsonb,
    base_sha                 text                     NOT NULL,
    head_shas                text[]                   NOT NULL,
    tested_pr_numbers        bigint[]                 NOT NULL,
    FOREIGN KEY (org_id, queue_id) REFERENCES queues (org_id, id),
    FOREIGN KEY (org_id, group_id) REFERENCES test_groups (org_id, id)
);

CREATE INDEX test_runs_live ON test_runs (queue_id, status)
    WHERE status IN ('assembling', 'in_progress');
-- The recent runs of a queue: the runs that tested a PR, for its history.
CREATE INDEX test_runs_queue_started ON test_runs (queue_id, started_at);
-- The run of a test commit, at each report of a check.
CREATE INDEX test_runs_test_sha ON test_runs (test_sha);
CREATE INDEX test_runs_group_id ON test_runs (group_id);
CREATE INDEX test_runs_succeeded ON test_runs (queue_id, ended_at) WHERE status = 'succeeded';

SELECT cihub_tenant_table('test_runs');
