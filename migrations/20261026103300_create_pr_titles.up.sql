-- The title of a PR, kept from the last read of the PR. Not in the log: a
-- title decides nothing, and a replay keeps it.
CREATE TABLE pr_titles (
    org_id    text        NOT NULL REFERENCES orgs (id),
    repo_id   text        NOT NULL,
    pr_number bigint      NOT NULL,
    title     text        NOT NULL,
    read_at   timestamptz NOT NULL,
    PRIMARY KEY (repo_id, pr_number),
    FOREIGN KEY (org_id, repo_id) REFERENCES repos (org_id, id)
);

SELECT cihub_tenant_table('pr_titles');
