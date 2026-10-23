CREATE TYPE repo_config_state AS ENUM ('none', 'valid', 'invalid');

-- The last setup read of a repository: the rules of its default branch, whose
-- configuration file `config_files` keeps. A cache of reads of the SCM, outside
-- the log: no state of a queue depends on it.
CREATE TABLE repo_setups (
    org_id          text        NOT NULL REFERENCES orgs (id),
    repo_id         text        PRIMARY KEY,
    -- The default branch that was read.
    branch          text        NOT NULL,
    -- `null` when the branch is missing.
    head_sha        text,
    -- `code` and `message` of each finding, as `queue_setups`.
    findings        jsonb       NOT NULL,
    -- The checks that the rules of the branch require; `null` when the branch
    -- is missing.
    required_checks text[],
    checked_at      timestamptz NOT NULL,
    FOREIGN KEY (org_id, repo_id) REFERENCES repos (org_id, id)
);

SELECT cihub_tenant_table('repo_setups');

-- The last read of the configuration file of a branch: the default branch of a
-- repository, read with its setup, and the target branch of each queue, read at
-- each new head. A cache of reads of the SCM, outside the log: a queue takes its
-- settings from `queues.config_text`, its last valid file.
CREATE TABLE config_files (
    org_id       text              NOT NULL REFERENCES orgs (id),
    repo_id      text              NOT NULL,
    branch       text              NOT NULL,
    -- `null` when the branch is missing.
    head_sha     text,
    state        repo_config_state NOT NULL,
    path         text,
    -- The file as read, valid or not.
    text         text,
    error        text,
    error_line   integer,
    error_column integer,
    -- The `branch` of each entry of a valid file.
    entries      text[]            NOT NULL DEFAULT '{}',
    read_at      timestamptz       NOT NULL,
    PRIMARY KEY (repo_id, branch),
    FOREIGN KEY (org_id, repo_id) REFERENCES repos (org_id, id)
);

SELECT cihub_tenant_table('config_files');

CREATE TYPE config_proposal_state AS ENUM ('requested', 'opened', 'merged', 'closed', 'failed');
CREATE TYPE config_proposal_reason AS ENUM ('config_changed', 'config_branch_refused',
                                            'pr_not_opened');

-- A proposal of a change of the configuration file of a branch: the PR that
-- the job `propose_config` opens, and how it ends. Outside the log: no state of
-- a queue depends on it.
CREATE TABLE config_proposals (
    id           text                   PRIMARY KEY,
    org_id       text                   NOT NULL REFERENCES orgs (id),
    repo_id      text                   NOT NULL,
    branch       text                   NOT NULL,
    -- The head that the person read: the file at the head of the branch must
    -- still be the one at `base_sha`.
    base_sha     text                   NOT NULL,
    path         text                   NOT NULL,
    text         text                   NOT NULL,
    message      text                   NOT NULL,
    -- `NULL` for an API token.
    requested_by text                   REFERENCES users (id),
    state        config_proposal_state  NOT NULL,
    state_reason config_proposal_reason,
    pr_number    bigint,
    requested_at timestamptz            NOT NULL,
    updated_at   timestamptz            NOT NULL,
    FOREIGN KEY (org_id, repo_id) REFERENCES repos (org_id, id)
);

-- One proposal of a branch that has not ended.
CREATE UNIQUE INDEX config_proposals_open ON config_proposals (repo_id, branch)
    WHERE state IN ('requested', 'opened');

SELECT cihub_tenant_table('config_proposals');

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
