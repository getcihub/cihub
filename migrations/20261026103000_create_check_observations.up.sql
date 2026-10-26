CREATE TYPE check_status AS ENUM ('queued', 'in_progress', 'completed');

CREATE TABLE check_observations (
    org_id       text             NOT NULL REFERENCES orgs (id),
    run_id       text             NOT NULL,
    name         text             NOT NULL,
    kind         check_kind       NOT NULL,
    required     boolean          NOT NULL,
    status       check_status     NOT NULL,
    conclusion   check_conclusion,
    details_url  text,
    started_at   timestamptz,
    completed_at timestamptz,
    observed_at  timestamptz      NOT NULL,
    PRIMARY KEY (run_id, name, kind)
);

SELECT cihub_tenant_table('check_observations');
