-- No row carries the two removed values: the casts cannot fail.
ALTER TYPE reason RENAME TO reason_before;

CREATE TYPE reason AS ENUM (
    'submitted', 'merge_item_now_ready', 'test_run_started', 'failure_budget_retry',
    'manual_push_to_target_branch', 'prerequisite_merge_item_failed',
    'prerequisite_merge_item_cancelled', 'prerequisite_merge_item_manually_merged',
    'batch_member_left', 'tests_restarted_by_user', 'tests_restarted_by_api_request',
    'skip_the_line_interrupted', 'bisection_required', 'bisection_test_run_passed',
    'test_run_passed', 'successor_test_run_passed', 'passed_by_optimization', 'merge_graph_merge',
    'merge_graph_direct_merge', 'manual_merge', 'user_requested', 'admin_requested', 'pr_closed',
    'pr_pushed_to', 'pr_base_branch_changed', 'pr_converted_to_draft', 'label_removed',
    'not_ready_timeout_expired', 'start_test_run_invalid_event', 'start_test_run_merge_conflict',
    'start_test_run_config_parsing_failure', 'start_test_run_config_bad_version',
    'start_test_run_config_missing_required_statuses',
    'start_test_run_config_bad_required_statuses', 'start_test_run_no_required_statuses',
    'start_test_run_retry_exhausted', 'start_test_run_draft_prs_not_allowed',
    'start_test_run_permissions_block_test_branch', 'test_run_required_status_failed',
    'test_run_timeout', 'merge_rejected', 'direct_merge_rejected', 'merge_rate_limit_exceeded',
    'merge_branch_protection_unmet', 'merge_already_in_progress', 'merge_repeated_server_errors',
    'pr_updated_at_merge_time', 'test_branch_assembled', 'mode_changed', 'mode_switch_completed',
    'queue_draining', 'queue_switching_modes'
);

ALTER TABLE queue_items ALTER COLUMN state_reason TYPE reason USING state_reason::text::reason;
ALTER TABLE test_runs ALTER COLUMN status_reason TYPE reason USING status_reason::text::reason;
ALTER TABLE an_item_conclusions ALTER COLUMN reason TYPE reason USING reason::text::reason;
ALTER TABLE an_state_durations ALTER COLUMN exit_reason TYPE reason USING exit_reason::text::reason;
ALTER TABLE an_test_run_results ALTER COLUMN reason TYPE reason USING reason::text::reason;
ALTER TABLE an_hourly_conclusions ALTER COLUMN reason TYPE reason USING reason::text::reason;

DROP TYPE reason_before;
