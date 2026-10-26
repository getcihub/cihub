CREATE TYPE event_type AS ENUM (
    'item.submitted', 'item.cancel_requested', 'item.restart_requested', 'pr.observed',
    'target.pushed', 'check.observed', 'targets.received', 'test_branch.assembled',
    'test_branch.assembly_failed', 'merge.succeeded', 'merge.failed', 'queue.control_requested',
    'queue.settings_changed', 'access.lost', 'access.restored', 'item.transitioned',
    'item.readiness_changed', 'queue.renumbered', 'group.formed', 'group.transitioned',
    'run.transitioned', 'queue.transitioned', 'queue.window_changed'
);
