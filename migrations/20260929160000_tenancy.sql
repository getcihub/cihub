-- Tenancy: SCM instances, organizations, their SCM accounts and connections,
-- repositories, users, memberships, API tokens and the audit log.
--
-- No column depends on one SCM (DECISIONS.md D46): an identifier of the SCM is
-- an `external_id` in text, unique within its SCM instance only.
--
-- Isolation (DECISIONS.md D38): every table that carries org_id has row level
-- security, forced on the owner too. A transaction sees the rows of the
-- organization in `cihub.org_id`, or every row when `cihub.system` is `on`;
-- without either setting it sees nothing. `scm_instances` and `users` are
-- global: organizations share an instance, and a user belongs to several
-- organizations through `memberships`.

CREATE FUNCTION cihub_org_id() RETURNS text
LANGUAGE sql STABLE
AS $$ SELECT nullif(current_setting('cihub.org_id', true), '') $$;

CREATE FUNCTION cihub_system() RETURNS boolean
LANGUAGE sql STABLE
AS $$ SELECT coalesce(current_setting('cihub.system', true), '') = 'on' $$;

CREATE TYPE membership_role AS ENUM ('admin', 'member', 'viewer');
CREATE TYPE scm_kind AS ENUM ('github');
CREATE TYPE connection_kind AS ENUM ('github_app', 'github_sudo_app');
CREATE TYPE api_scope AS ENUM (
    'queues:read', 'queues:write', 'prs:write', 'targets:write', 'metrics:read'
);

CREATE TABLE scm_instances (
    id         text        PRIMARY KEY,
    kind       scm_kind    NOT NULL,
    base_url   text        NOT NULL UNIQUE,
    created_at timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE orgs (
    id         text        PRIMARY KEY,
    slug       text        NOT NULL UNIQUE,
    created_at timestamptz NOT NULL DEFAULT now()
);

-- An account of the SCM (a GitHub organization) linked to an organization.
CREATE TABLE scm_accounts (
    id              text        PRIMARY KEY,
    org_id          text        NOT NULL REFERENCES orgs (id),
    scm_instance_id text        NOT NULL REFERENCES scm_instances (id),
    external_id     text        NOT NULL,
    path            text        NOT NULL,
    created_at      timestamptz NOT NULL DEFAULT now(),
    UNIQUE (scm_instance_id, external_id),
    UNIQUE (org_id, scm_instance_id, id)
);

CREATE INDEX scm_accounts_org_id ON scm_accounts (org_id);

-- How CIHub reaches the repositories of an account: a GitHub App installation.
CREATE TABLE scm_connections (
    id              text            PRIMARY KEY,
    org_id          text            NOT NULL REFERENCES orgs (id),
    scm_instance_id text            NOT NULL,
    scm_account_id  text            NOT NULL,
    kind            connection_kind NOT NULL,
    external_id     text            NOT NULL,
    suspended_at    timestamptz,
    claimed_at      timestamptz,
    created_at      timestamptz     NOT NULL DEFAULT now(),
    UNIQUE (scm_instance_id, kind, external_id),
    UNIQUE (org_id, scm_instance_id, id),
    -- A connection, its account and their instance belong together.
    FOREIGN KEY (org_id, scm_instance_id, scm_account_id)
        REFERENCES scm_accounts (org_id, scm_instance_id, id)
);

CREATE INDEX scm_connections_org_id ON scm_connections (org_id);

CREATE TABLE repos (
    id              text        PRIMARY KEY,
    org_id          text        NOT NULL REFERENCES orgs (id),
    scm_instance_id text        NOT NULL,
    connection_id   text        NOT NULL,
    external_id     text        NOT NULL,
    namespace       text        NOT NULL,
    name            text        NOT NULL,
    default_branch  text        NOT NULL,
    created_at      timestamptz NOT NULL DEFAULT now(),
    UNIQUE (scm_instance_id, external_id),
    -- A repository, its connection and their instance belong together.
    FOREIGN KEY (org_id, scm_instance_id, connection_id)
        REFERENCES scm_connections (org_id, scm_instance_id, id)
);

CREATE INDEX repos_org_id ON repos (org_id);

-- An account of an SCM user; several accounts of one person are several rows.
CREATE TABLE users (
    id              text        PRIMARY KEY,
    scm_instance_id text        NOT NULL REFERENCES scm_instances (id),
    external_id     text        NOT NULL,
    login           text        NOT NULL,
    created_at      timestamptz NOT NULL DEFAULT now(),
    UNIQUE (scm_instance_id, external_id)
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
    -- SHA-256 of the whole token `cht_…` (DECISIONS.md D32).
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

ALTER TABLE scm_accounts ENABLE ROW LEVEL SECURITY;
ALTER TABLE scm_accounts FORCE ROW LEVEL SECURITY;
CREATE POLICY tenant ON scm_accounts
    USING (cihub_system() OR org_id = cihub_org_id())
    WITH CHECK (cihub_system() OR org_id = cihub_org_id());

ALTER TABLE scm_connections ENABLE ROW LEVEL SECURITY;
ALTER TABLE scm_connections FORCE ROW LEVEL SECURITY;
CREATE POLICY tenant ON scm_connections
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
