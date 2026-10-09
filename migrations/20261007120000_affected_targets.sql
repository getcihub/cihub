-- Lot 13. The affected targets that the CI of a PR uploads (DECISIONS.md D234),
-- kept by head: the CI often uploads before the submission of the PR.
CREATE TABLE affected_targets (
    org_id      text        NOT NULL REFERENCES orgs (id),
    repo_id     text        NOT NULL,
    pr_number   bigint      NOT NULL,
    head_sha    text        NOT NULL,
    -- Empty when `is_all`.
    targets     text[]      NOT NULL,
    is_all      boolean     NOT NULL,
    uploaded_at timestamptz NOT NULL,
    PRIMARY KEY (repo_id, pr_number, head_sha),
    FOREIGN KEY (org_id, repo_id) REFERENCES repos (org_id, id)
);

SELECT cihub_tenant_table('affected_targets');
