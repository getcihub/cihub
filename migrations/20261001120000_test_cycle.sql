-- The test cycle (lot 6): the checks observed on the test SHA of a run
-- (docs/DATA_MODEL.md).

CREATE TYPE check_kind AS ENUM ('check', 'status');
CREATE TYPE check_conclusion AS ENUM (
    'success', 'neutral', 'skipped', 'failure', 'error', 'cancelled', 'timed_out',
    'action_required', 'stale'
);

-- The last report of each check or commit status on the test SHA of a run; a
-- required one decides the run. A report that comes after the end of the run
-- is stored here without a fact in the log (D118); no foreign key to
-- `test_runs`, so that a replay of the log, which writes the runs again, keeps
-- these rows.
CREATE TABLE check_observations (
    org_id      text             NOT NULL REFERENCES orgs (id),
    run_id      text             NOT NULL,
    name        text             NOT NULL,
    kind        check_kind       NOT NULL,
    -- The name is in the required statuses of the run.
    required    boolean          NOT NULL,
    -- `NULL` while the check has not concluded.
    conclusion  check_conclusion,
    details_url text,
    observed_at timestamptz      NOT NULL,
    PRIMARY KEY (run_id, name, kind)
);

SELECT cihub_tenant_table('check_observations');
