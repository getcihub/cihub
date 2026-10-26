CREATE TYPE check_conclusion AS ENUM (
    'success', 'neutral', 'skipped', 'failure', 'error', 'cancelled', 'timed_out',
    'action_required', 'stale'
);
