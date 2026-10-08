CREATE TYPE module_name AS ENUM ('mergequeue');
CREATE TYPE item_state AS ENUM (
    'not_ready', 'pending', 'testing', 'tests_passed', 'pending_failure', 'merged', 'failed',
    'cancelled'
);
CREATE TYPE group_state AS ENUM (
    'testing', 'pending_failure', 'tests_passed', 'merged', 'failed', 'bisected', 'dissolved'
);
CREATE TYPE run_status AS ENUM (
    'assembling', 'in_progress', 'succeeded', 'failed', 'interrupted', 'cancelled', 'timed_out'
);
CREATE TYPE submitted_via AS ENUM ('comment', 'label', 'api', 'ui', 'cli');
CREATE TYPE required_statuses_source AS ENUM ('repo_config', 'branch_protection');
CREATE TYPE reason AS ENUM (
    'submitted', 'merge_item_now_ready', 'test_run_started', 'failure_budget_retry',
    'manual_push_to_target_branch', 'prerequisite_merge_item_failed',
    'prerequisite_merge_item_cancelled', 'prerequisite_merge_item_manually_merged',
    'batch_member_left',
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
    'test_run_timeout', 'merge_rejected',
    'direct_merge_rejected', 'merge_rate_limit_exceeded', 'merge_branch_protection_unmet',
    'merge_already_in_progress', 'merge_repeated_server_errors', 'pr_updated_at_merge_time',
    'test_branch_assembled', 'access_lost', 'access_restored', 'mode_changed',
    'mode_switch_completed', 'queue_draining', 'queue_switching_modes'
);
CREATE TYPE permission AS ENUM ('none', 'read', 'write', 'admin');

CREATE TABLE org_modules (
    org_id     text        NOT NULL REFERENCES orgs (id),
    module     module_name NOT NULL,
    enabled_at timestamptz NOT NULL,
    PRIMARY KEY (org_id, module)
);

CREATE TABLE queue_items (
    id                 text          PRIMARY KEY,
    org_id             text          NOT NULL REFERENCES orgs (id),
    queue_id           text          NOT NULL,
    pr_number          bigint        NOT NULL,
    pr_head_sha        text          NOT NULL,
    pr_author_id       text          NOT NULL REFERENCES users (id),
    state              item_state    NOT NULL,
    state_reason       reason        NOT NULL,
    state_changed_at   timestamptz   NOT NULL,
    position           bigint,
    priority_value     smallint      NOT NULL,
    no_batch           boolean       NOT NULL,
    force_enqueued     boolean       NOT NULL,
    submitted_by       text          REFERENCES users (id),
    submitted_via      submitted_via NOT NULL,
    sender_permission  permission,
    submitted_at       timestamptz   NOT NULL,
    block_reasons      jsonb         NOT NULL,
    group_id           text,
    is_current         boolean       NOT NULL,
    verified_by_run_id text,
    merged_sha         text,
    FOREIGN KEY (org_id, queue_id) REFERENCES queues (org_id, id)
);

CREATE UNIQUE INDEX queue_items_current ON queue_items (queue_id, pr_number) WHERE is_current;
CREATE INDEX queue_items_pr ON queue_items (queue_id, pr_number);
CREATE INDEX queue_items_order ON queue_items (queue_id, position)
    WHERE is_current AND state NOT IN ('merged', 'failed', 'cancelled');

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
    base_sha                 text                     NOT NULL,
    head_shas                text[]                   NOT NULL,
    tested_pr_numbers        bigint[]                 NOT NULL,
    last_report_at           timestamptz,
    FOREIGN KEY (org_id, queue_id) REFERENCES queues (org_id, id),
    FOREIGN KEY (org_id, group_id) REFERENCES test_groups (org_id, id)
);

CREATE INDEX test_runs_live ON test_runs (queue_id, status)
    WHERE status IN ('assembling', 'in_progress');
CREATE INDEX test_runs_tested_pr_numbers ON test_runs USING gin (tested_pr_numbers);

CREATE TABLE queue_snapshots (
    org_id     text        NOT NULL,
    queue_id   text        NOT NULL,
    seq        bigint      NOT NULL CHECK (seq > 0),
    payload    jsonb       NOT NULL,
    created_at timestamptz NOT NULL,
    PRIMARY KEY (queue_id, seq),
    FOREIGN KEY (org_id, queue_id) REFERENCES queues (org_id, id)
);

CREATE TABLE queue_inbox (
    id            text        PRIMARY KEY,
    org_id        text        NOT NULL REFERENCES orgs (id),
    queue_id      text        NOT NULL,
    actor_kind    actor_kind  NOT NULL,
    actor_user_id text        REFERENCES users (id),
    type          event_type  NOT NULL,
    payload       jsonb       NOT NULL,
    received_at   timestamptz NOT NULL,
    arrival       bigint      GENERATED ALWAYS AS IDENTITY,
    processed_at  timestamptz,
    FOREIGN KEY (org_id, queue_id) REFERENCES queues (org_id, id),
    CHECK ((actor_kind = 'user') = (actor_user_id IS NOT NULL))
);

CREATE INDEX queue_inbox_pending ON queue_inbox (queue_id, arrival)
    WHERE processed_at IS NULL;

SELECT cihub_tenant_table('org_modules');
SELECT cihub_tenant_table('queue_items');
SELECT cihub_tenant_table('test_groups');
SELECT cihub_tenant_table('test_runs');
SELECT cihub_tenant_table('queue_snapshots');
SELECT cihub_tenant_table('queue_inbox');
