CREATE TABLE pr_comments (
    org_id              text   NOT NULL REFERENCES orgs (id),
    repo_id             text   NOT NULL,
    pr_number           bigint NOT NULL,
    comment_external_id text   NOT NULL,
    PRIMARY KEY (repo_id, pr_number),
    FOREIGN KEY (org_id, repo_id) REFERENCES repos (org_id, id)
);

SELECT cihub_tenant_table('pr_comments');
