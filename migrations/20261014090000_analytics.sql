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

-- The facts are derived from the log only, so that a replay writes them again.
CREATE TABLE an_item_conclusions (
    item_id      text        PRIMARY KEY,
    org_id       text        NOT NULL REFERENCES orgs (id),
    queue_id     text        NOT NULL,
    conclusion   item_state  NOT NULL CHECK (conclusion IN ('merged', 'failed', 'cancelled')),
    reason       reason      NOT NULL,
    submitted_at timestamptz NOT NULL,
    concluded_at timestamptz NOT NULL,
    FOREIGN KEY (org_id, queue_id) REFERENCES queues (org_id, id)
);

CREATE INDEX an_item_conclusions_queue ON an_item_conclusions (queue_id, concluded_at);

SELECT cihub_tenant_table('an_item_conclusions');

-- One row per pass of a concluded item through a state; `pass` counts them
-- from 0, as one reconciliation can move an item twice at the same time.
CREATE TABLE an_state_durations (
    org_id      text        NOT NULL REFERENCES orgs (id),
    queue_id    text        NOT NULL,
    item_id     text        NOT NULL REFERENCES an_item_conclusions (item_id),
    pass        integer     NOT NULL,
    state       item_state  NOT NULL,
    entered_at  timestamptz NOT NULL,
    left_at     timestamptz NOT NULL,
    exit_reason reason      NOT NULL,
    PRIMARY KEY (item_id, pass),
    FOREIGN KEY (org_id, queue_id) REFERENCES queues (org_id, id)
);

SELECT cihub_tenant_table('an_state_durations');

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

-- The last report of each check on the test commit of an ended run;
-- `passed_on_retry` when it passed after it failed on the attempt before.
CREATE TABLE an_check_results (
    org_id          text             NOT NULL REFERENCES orgs (id),
    run_id          text             NOT NULL REFERENCES an_test_run_results (run_id),
    name            text             NOT NULL,
    kind            check_kind       NOT NULL,
    required        boolean          NOT NULL,
    conclusion      check_conclusion,
    passed_on_retry boolean          NOT NULL,
    PRIMARY KEY (run_id, name, kind)
);

SELECT cihub_tenant_table('an_check_results');

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

-- A histogram of the durations of the merged PRs: the time to merge, and the
-- time in each live state, 0 for a state that a PR skipped. Bucket 0 holds no
-- time; bucket `b` the durations from 1.1^(b - 1) to 1.1^b seconds, so that a
-- percentile read from it is within 5 % of the exact one.
CREATE TABLE an_hourly_durations (
    org_id   text        NOT NULL REFERENCES orgs (id),
    queue_id text        NOT NULL,
    hour     timestamptz NOT NULL,
    measure  text        NOT NULL CHECK (measure IN ('time_to_merge', 'not_ready', 'pending',
                                                     'testing', 'pending_failure',
                                                     'tests_passed')),
    bucket   smallint    NOT NULL CHECK (bucket >= 0),
    count    bigint      NOT NULL,
    PRIMARY KEY (queue_id, hour, measure, bucket),
    FOREIGN KEY (org_id, queue_id) REFERENCES queues (org_id, id)
);

SELECT cihub_tenant_table('an_hourly_durations');

CREATE TABLE an_hourly_checks (
    org_id          text        NOT NULL REFERENCES orgs (id),
    queue_id        text        NOT NULL,
    hour            timestamptz NOT NULL,
    name            text        NOT NULL,
    kind            check_kind  NOT NULL,
    required        boolean     NOT NULL,
    runs            bigint      NOT NULL,
    failed          bigint      NOT NULL,
    passed_on_retry bigint      NOT NULL,
    PRIMARY KEY (queue_id, hour, name, kind),
    FOREIGN KEY (org_id, queue_id) REFERENCES queues (org_id, id)
);

SELECT cihub_tenant_table('an_hourly_checks');
