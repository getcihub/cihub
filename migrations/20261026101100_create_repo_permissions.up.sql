-- What a person may do on a repository, as the SCM gave it with their token,
-- and when: a session reads only the repositories where the person has
-- `read`, and checks a permission older than an hour again on the SCM.
CREATE TABLE repo_permissions (
    org_id     text        NOT NULL REFERENCES orgs (id),
    repo_id    text        NOT NULL,
    user_id    text        NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    permission permission  NOT NULL,
    checked_at timestamptz NOT NULL,
    PRIMARY KEY (repo_id, user_id),
    FOREIGN KEY (org_id, repo_id) REFERENCES repos (org_id, id)
);

CREATE INDEX repo_permissions_user ON repo_permissions (user_id);

SELECT cihub_tenant_table('repo_permissions');
