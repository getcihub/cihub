CREATE TYPE run_status AS ENUM (
    'assembling', 'in_progress', 'succeeded', 'failed', 'interrupted', 'cancelled', 'timed_out'
);
