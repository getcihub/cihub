-- Tenancy: organizations, GitHub App installations, repositories, users,
-- memberships, API tokens and the audit log.
--
-- Isolation (AGENTS.md D38): every table that carries org_id has row level
-- security, forced on the owner too. A transaction sees the rows of the
-- organization in `cihub.org_id`, or every row when `cihub.system` is `on`;
-- without either setting it sees nothing. `users` is global: a GitHub user
-- belongs to several organizations through `memberships`.

CREATE FUNCTION cihub_org_id() RETURNS text
LANGUAGE sql STABLE
AS $$ SELECT nullif(current_setting('cihub.org_id', true), '') $$;

CREATE FUNCTION cihub_system() RETURNS boolean
LANGUAGE sql STABLE
AS $$ SELECT coalesce(current_setting('cihub.system', true), '') = 'on' $$;

CREATE TYPE membership_role AS ENUM ('admin', 'member', 'viewer');
CREATE TYPE app_kind AS ENUM ('main', 'sudo');
CREATE TYPE api_scope AS ENUM (
    'queues:read', 'queues:write', 'prs:write', 'targets:write', 'metrics:read'
);

CREATE TABLE orgs (
    id            text        PRIMARY KEY,
    slug          text        NOT NULL UNIQUE,
    github_org_id bigint      NOT NULL UNIQUE,
    settings      jsonb       NOT NULL DEFAULT '{}',
    created_at    timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE installations (
    id                     text        PRIMARY KEY,
    org_id                 text        NOT NULL REFERENCES orgs (id),
    github_installation_id bigint      NOT NULL,
    app_kind               app_kind    NOT NULL,
    account_type           text        NOT NULL,
    suspended_at           timestamptz,
    claimed_at             timestamptz,
    created_at             timestamptz NOT NULL DEFAULT now(),
    UNIQUE (app_kind, github_installation_id),
    UNIQUE (org_id, id)
);
CREATE INDEX installations_org_id ON installations (org_id);

CREATE TABLE repos (
    id              text        PRIMARY KEY,
    org_id          text        NOT NULL REFERENCES orgs (id),
    installation_id text        NOT NULL,
    github_repo_id  bigint      NOT NULL UNIQUE,
    owner           text        NOT NULL,
    name            text        NOT NULL,
    default_branch  text        NOT NULL,
    created_at      timestamptz NOT NULL DEFAULT now(),
    -- A repository and its installation belong to the same organization.
    FOREIGN KEY (org_id, installation_id) REFERENCES installations (org_id, id)
);
CREATE INDEX repos_org_id ON repos (org_id);

CREATE TABLE users (
    id             text        PRIMARY KEY,
    github_user_id bigint      NOT NULL UNIQUE,
    login          text        NOT NULL,
    created_at     timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE memberships (
    org_id     text            NOT NULL REFERENCES orgs (id),
    user_id    text            NOT NULL REFERENCES users (id),
    role       membership_role NOT NULL,
    created_at timestamptz     NOT NULL DEFAULT now(),
    PRIMARY KEY (org_id, user_id)
);

CREATE TABLE api_tokens (
    id           text        PRIMARY KEY,
    org_id       text        NOT NULL REFERENCES orgs (id),
    name         text        NOT NULL,
    -- SHA-256 of the whole token `cht_…` (AGENTS.md D32).
    token_hash   bytea       NOT NULL UNIQUE CHECK (length(token_hash) = 32),
    scopes       api_scope[] NOT NULL,
    created_by   text        REFERENCES users (id),
    created_at   timestamptz NOT NULL DEFAULT now(),
    expires_at   timestamptz,
    last_used_at timestamptz
);
CREATE INDEX api_tokens_org_id ON api_tokens (org_id);

CREATE TABLE audit_log (
    id            text        PRIMARY KEY,
    org_id        text        NOT NULL REFERENCES orgs (id),
    actor_user_id text        REFERENCES users (id),
    action        text        NOT NULL,
    target        text        NOT NULL,
    before        jsonb,
    after         jsonb,
    ip            inet,
    occurred_at   timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX audit_log_org_id_occurred_at ON audit_log (org_id, occurred_at);

ALTER TABLE orgs ENABLE ROW LEVEL SECURITY;
ALTER TABLE orgs FORCE ROW LEVEL SECURITY;
CREATE POLICY tenant ON orgs
    USING (cihub_system() OR id = cihub_org_id())
    WITH CHECK (cihub_system() OR id = cihub_org_id());

ALTER TABLE installations ENABLE ROW LEVEL SECURITY;
ALTER TABLE installations FORCE ROW LEVEL SECURITY;
CREATE POLICY tenant ON installations
    USING (cihub_system() OR org_id = cihub_org_id())
    WITH CHECK (cihub_system() OR org_id = cihub_org_id());

ALTER TABLE repos ENABLE ROW LEVEL SECURITY;
ALTER TABLE repos FORCE ROW LEVEL SECURITY;
CREATE POLICY tenant ON repos
    USING (cihub_system() OR org_id = cihub_org_id())
    WITH CHECK (cihub_system() OR org_id = cihub_org_id());

ALTER TABLE memberships ENABLE ROW LEVEL SECURITY;
ALTER TABLE memberships FORCE ROW LEVEL SECURITY;
CREATE POLICY tenant ON memberships
    USING (cihub_system() OR org_id = cihub_org_id())
    WITH CHECK (cihub_system() OR org_id = cihub_org_id());

ALTER TABLE api_tokens ENABLE ROW LEVEL SECURITY;
ALTER TABLE api_tokens FORCE ROW LEVEL SECURITY;
CREATE POLICY tenant ON api_tokens
    USING (cihub_system() OR org_id = cihub_org_id())
    WITH CHECK (cihub_system() OR org_id = cihub_org_id());

ALTER TABLE audit_log ENABLE ROW LEVEL SECURITY;
ALTER TABLE audit_log FORCE ROW LEVEL SECURITY;
CREATE POLICY tenant ON audit_log
    USING (cihub_system() OR org_id = cihub_org_id())
    WITH CHECK (cihub_system() OR org_id = cihub_org_id());
