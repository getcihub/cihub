CREATE TYPE repo_config_state AS ENUM ('none', 'valid', 'invalid');

-- The last setup read of a repository: the rules of its default branch and its
-- configuration file. A cache of reads of the SCM, outside the log: no state
-- of a queue depends on it.
CREATE TABLE repo_setups (
    org_id        text              NOT NULL REFERENCES orgs (id),
    repo_id       text              PRIMARY KEY,
    -- The default branch that was read.
    branch        text              NOT NULL,
    -- `null` when the branch is missing.
    head_sha      text,
    -- `code` and `message` of each finding, as `queue_setups`.
    findings      jsonb             NOT NULL,
    config_state  repo_config_state NOT NULL,
    config_path   text,
    config_error  text,
    config_line   integer,
    config_column integer,
    -- The file as read: the API gives the strategy and the settings of the
    -- default branch from it, and its text only when it is not valid.
    config_text   text,
    entries       text[]            NOT NULL DEFAULT '{}',
    checked_at    timestamptz       NOT NULL,
    FOREIGN KEY (org_id, repo_id) REFERENCES repos (org_id, id)
);

SELECT cihub_tenant_table('repo_setups');

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
