CREATE TYPE item_state AS ENUM (
    'not_ready', 'pending', 'testing', 'tests_passed', 'pending_failure', 'merged', 'failed',
    'cancelled'
);
