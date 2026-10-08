CREATE TYPE check_kind AS ENUM ('check', 'status');
CREATE TYPE check_conclusion AS ENUM (
    'success', 'neutral', 'skipped', 'failure', 'error', 'cancelled', 'timed_out',
    'action_required', 'stale'
);

CREATE TABLE check_observations (
    org_id      text             NOT NULL REFERENCES orgs (id),
    run_id      text             NOT NULL,
    name        text             NOT NULL,
    kind        check_kind       NOT NULL,
    required    boolean          NOT NULL,
    conclusion  check_conclusion,
    details_url text,
    observed_at timestamptz      NOT NULL,
    PRIMARY KEY (run_id, name, kind)
);

SELECT cihub_tenant_table('check_observations');
