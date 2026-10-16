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
    -- Only when the file is not valid.
    config_text   text,
    entries       text[]            NOT NULL DEFAULT '{}',
    checked_at    timestamptz       NOT NULL,
    FOREIGN KEY (org_id, repo_id) REFERENCES repos (org_id, id)
);

SELECT cihub_tenant_table('repo_setups');
