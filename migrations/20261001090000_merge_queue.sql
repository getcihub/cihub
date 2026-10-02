-- The merge queue module (DECISIONS.md D94, D95, D100): the modules of an
-- organization, the state and the mode of a queue, the projections of the log
-- (`queue_items`, `test_groups`, `test_runs`, D28), the snapshots of the log,
-- and the inbox of the facts that the jobs give back to a queue.
--
-- The projections hold only what the log holds: a replay of the log rebuilds
-- them exactly (principle I). A list of identifiers is in `text[]` (D37).

CREATE TYPE module_name AS ENUM ('mergequeue');
CREATE TYPE queue_state AS ENUM ('running', 'paused', 'draining', 'switching_modes');
CREATE TYPE queue_mode AS ENUM ('single', 'parallel');
CREATE TYPE item_state AS ENUM (
    'not_ready', 'pending', 'testing', 'tests_passed', 'pending_failure', 'merged', 'failed',
    'cancelled'
);
CREATE TYPE group_state AS ENUM (
    'testing', 'pending_failure', 'tests_passed', 'merged', 'failed', 'bisected', 'dissolved'
);
CREATE TYPE group_kind AS ENUM ('single', 'batch', 'bisection');
CREATE TYPE run_status AS ENUM (
    'assembling', 'in_progress', 'succeeded', 'failed', 'interrupted', 'cancelled', 'timed_out'
);
CREATE TYPE submitted_via AS ENUM ('comment', 'label', 'api', 'ui', 'cli');
CREATE TYPE mergeability AS ENUM ('mergeable', 'behind', 'conflict', 'blocked', 'unknown');
CREATE TYPE required_statuses_source AS ENUM ('repo_config', 'branch_protection');
CREATE TYPE reason AS ENUM (
    'submitted', 'merge_item_now_ready', 'test_run_started', 'failure_budget_retry',
    'manual_push_to_target_branch', 'prerequisite_merge_item_failed',
    'prerequisite_merge_item_cancelled', 'prerequisite_merge_item_manually_merged',
    'tests_restarted_by_user', 'tests_restarted_by_api_request', 'skip_the_line_interrupted',
    'bisection_required', 'bisection_test_run_passed', 'test_run_passed',
    'successor_test_run_passed', 'passed_by_optimization', 'merge_graph_merge',
    'merge_graph_direct_merge', 'manual_merge', 'user_requested', 'admin_requested', 'pr_closed',
    'pr_pushed_to', 'pr_base_branch_changed', 'pr_converted_to_draft', 'label_removed',
    'not_ready_timeout_expired', 'start_test_run_invalid_event', 'start_test_run_merge_conflict',
    'start_test_run_config_not_found', 'start_test_run_config_parsing_failure',
    'start_test_run_config_bad_version', 'start_test_run_config_missing_required_statuses',
    'start_test_run_config_bad_required_statuses', 'start_test_run_no_required_statuses',
    'start_test_run_retry_exhausted', 'start_test_run_draft_prs_not_allowed',
    'start_test_run_permissions_block_test_branch', 'test_run_required_status_failed',
    'test_run_timeout', 'test_run_failed_by_optimization', 'merge_rejected',
    'direct_merge_rejected', 'merge_rate_limit_exceeded', 'merge_branch_protection_unmet',
    'merge_already_in_progress', 'merge_repeated_server_errors', 'pr_updated_at_merge_time',
    'test_branch_assembled', 'access_lost', 'access_restored', 'mode_changed',
    'mode_switch_completed'
);

-- An organization has a module from `enabled_at`; without a row, it does not
-- have it (D94).
CREATE TABLE org_modules (
    org_id     text        NOT NULL REFERENCES orgs (id),
    module     module_name NOT NULL,
    enabled_at timestamptz NOT NULL,
    PRIMARY KEY (org_id, module)
);

-- A projection of the log, like the tables below.
ALTER TABLE queues
    ADD COLUMN state queue_state NOT NULL DEFAULT 'running',
    ADD COLUMN mode  queue_mode  NOT NULL DEFAULT 'single';

CREATE TABLE queue_items (
    id                 text          PRIMARY KEY,
    org_id             text          NOT NULL REFERENCES orgs (id),
    queue_id           text          NOT NULL,
    pr_number          bigint        NOT NULL,
    pr_head_sha        text          NOT NULL,
    -- The author of the PR; `users` keeps the login (D42).
    pr_author_id       text          NOT NULL REFERENCES users (id),
    state              item_state    NOT NULL,
    state_reason       reason        NOT NULL,
    state_changed_at   timestamptz   NOT NULL,
    -- The rank in the log of the admission, the order of entry.
    seq                bigint,
    -- The queue order, by steps of 1 000 (docs/ALGORITHMS.md).
    position           bigint,
    priority_value     smallint      NOT NULL,
    priority_name      text,
    no_batch           boolean       NOT NULL,
    force_enqueued     boolean       NOT NULL,
    skip_the_line      boolean       NOT NULL,
    -- `NULL` when CIHub submitted the PR itself.
    submitted_by       text          REFERENCES users (id),
    submitted_via      submitted_via NOT NULL,
    submitted_at       timestamptz   NOT NULL,
    mergeability       mergeability  NOT NULL,
    block_reasons      jsonb         NOT NULL,
    group_id           text,
    is_current         boolean       NOT NULL,
    verified_by_run_id text,
    merged_sha         text,
    concluded_at       timestamptz,
    UNIQUE (org_id, id),
    FOREIGN KEY (org_id, queue_id) REFERENCES queues (org_id, id)
);

-- A PR has at most one live item; its history stays.
CREATE UNIQUE INDEX queue_items_current ON queue_items (queue_id, pr_number) WHERE is_current;
CREATE INDEX queue_items_order ON queue_items (queue_id, position)
    WHERE is_current AND state NOT IN ('merged', 'failed', 'cancelled');

CREATE TABLE test_groups (
    id              text        PRIMARY KEY,
    org_id          text        NOT NULL REFERENCES orgs (id),
    queue_id        text        NOT NULL,
    kind            group_kind  NOT NULL,
    -- In order.
    item_ids        text[]      NOT NULL,
    parent_group_id text,
    predecessor_ids text[]      NOT NULL,
    base_sha        text        NOT NULL,
    -- The rank in the log of the formation.
    seq             bigint      NOT NULL,
    state           group_state NOT NULL,
    state_reason    reason      NOT NULL,
    created_at      timestamptz NOT NULL,
    concluded_at    timestamptz,
    UNIQUE (org_id, id),
    FOREIGN KEY (org_id, queue_id) REFERENCES queues (org_id, id)
);

CREATE INDEX test_groups_queue_id ON test_groups (queue_id, seq);

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
    failure_reason           reason,
    failure_causing_statuses text[]                   NOT NULL,
    conflict_culprit         jsonb,
    FOREIGN KEY (org_id, queue_id) REFERENCES queues (org_id, id),
    FOREIGN KEY (org_id, group_id) REFERENCES test_groups (org_id, id)
);

CREATE INDEX test_runs_live ON test_runs (queue_id, status)
    WHERE status IN ('assembling', 'in_progress');

-- The state of a queue after the event `seq` of its log, every 500 events: a
-- reconciliation applies only the events after the last snapshot.
CREATE TABLE queue_snapshots (
    org_id     text        NOT NULL,
    queue_id   text        NOT NULL,
    seq        bigint      NOT NULL CHECK (seq > 0),
    -- The snapshot of `cihub-mq-core`, as it serializes it.
    payload    jsonb       NOT NULL,
    created_at timestamptz NOT NULL,
    PRIMARY KEY (queue_id, seq),
    FOREIGN KEY (org_id, queue_id) REFERENCES queues (org_id, id)
);

-- The facts that a job or the API gives back to a queue (D100). Only the
-- scheduler writes the log: it applies them under the lock of the queue.
CREATE TABLE queue_inbox (
    -- The identifier of the event in the log.
    id            text        PRIMARY KEY,
    org_id        text        NOT NULL REFERENCES orgs (id),
    queue_id      text        NOT NULL,
    actor_kind    actor_kind  NOT NULL,
    actor_user_id text        REFERENCES users (id),
    type          event_type  NOT NULL,
    -- The fields of the type, as `cihub-domain` serializes them.
    payload       jsonb       NOT NULL,
    received_at   timestamptz NOT NULL,
    -- The order of the deposits: the facts of one job have the same time, and
    -- their order counts (a submission before the read of its PR).
    arrival       bigint      GENERATED ALWAYS AS IDENTITY,
    processed_at  timestamptz,
    FOREIGN KEY (org_id, queue_id) REFERENCES queues (org_id, id),
    CHECK ((actor_kind = 'user') = (actor_user_id IS NOT NULL))
);

CREATE INDEX queue_inbox_pending ON queue_inbox (queue_id, arrival)
    WHERE processed_at IS NULL;

ALTER TABLE org_modules ENABLE ROW LEVEL SECURITY;
ALTER TABLE org_modules FORCE ROW LEVEL SECURITY;
CREATE POLICY tenant ON org_modules
    USING (cihub_system() OR org_id = cihub_org_id())
    WITH CHECK (cihub_system() OR org_id = cihub_org_id());

ALTER TABLE queue_items ENABLE ROW LEVEL SECURITY;
ALTER TABLE queue_items FORCE ROW LEVEL SECURITY;
CREATE POLICY tenant ON queue_items
    USING (cihub_system() OR org_id = cihub_org_id())
    WITH CHECK (cihub_system() OR org_id = cihub_org_id());

ALTER TABLE test_groups ENABLE ROW LEVEL SECURITY;
ALTER TABLE test_groups FORCE ROW LEVEL SECURITY;
CREATE POLICY tenant ON test_groups
    USING (cihub_system() OR org_id = cihub_org_id())
    WITH CHECK (cihub_system() OR org_id = cihub_org_id());

ALTER TABLE test_runs ENABLE ROW LEVEL SECURITY;
ALTER TABLE test_runs FORCE ROW LEVEL SECURITY;
CREATE POLICY tenant ON test_runs
    USING (cihub_system() OR org_id = cihub_org_id())
    WITH CHECK (cihub_system() OR org_id = cihub_org_id());

ALTER TABLE queue_snapshots ENABLE ROW LEVEL SECURITY;
ALTER TABLE queue_snapshots FORCE ROW LEVEL SECURITY;
CREATE POLICY tenant ON queue_snapshots
    USING (cihub_system() OR org_id = cihub_org_id())
    WITH CHECK (cihub_system() OR org_id = cihub_org_id());

ALTER TABLE queue_inbox ENABLE ROW LEVEL SECURITY;
ALTER TABLE queue_inbox FORCE ROW LEVEL SECURITY;
CREATE POLICY tenant ON queue_inbox
    USING (cihub_system() OR org_id = cihub_org_id())
    WITH CHECK (cihub_system() OR org_id = cihub_org_id());
