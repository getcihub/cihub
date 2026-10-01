-- The test cycle (lot 6): what a run tests, and the checks observed on its
-- test SHA (docs/DATA_MODEL.md).

CREATE TYPE check_kind AS ENUM ('check', 'status');
CREATE TYPE check_conclusion AS ENUM (
    'success', 'neutral', 'skipped', 'failure', 'error', 'cancelled', 'timed_out',
    'action_required', 'stale'
);

ALTER TABLE test_runs
    -- The head of the target branch that the run builds on: that of its group.
    ADD COLUMN base_sha          text        NOT NULL DEFAULT '',
    -- The heads merged on `base_sha`, in order: the predecessors, then the
    -- members of the group.
    ADD COLUMN head_shas         text[]      NOT NULL DEFAULT '{}',
    -- The PRs of the group.
    ADD COLUMN tested_pr_numbers bigint[]    NOT NULL DEFAULT '{}',
    -- The last report of a check on the test SHA, late reports included.
    ADD COLUMN last_report_at    timestamptz;
ALTER TABLE test_runs
    ALTER COLUMN base_sha DROP DEFAULT,
    ALTER COLUMN head_shas DROP DEFAULT,
    ALTER COLUMN tested_pr_numbers DROP DEFAULT;

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

ALTER TABLE check_observations ENABLE ROW LEVEL SECURITY;
ALTER TABLE check_observations FORCE ROW LEVEL SECURITY;
CREATE POLICY tenant ON check_observations
    USING (cihub_system() OR org_id = cihub_org_id())
    WITH CHECK (cihub_system() OR org_id = cihub_org_id());
