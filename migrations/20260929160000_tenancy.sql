CREATE FUNCTION cihub_org_id() RETURNS text
LANGUAGE sql STABLE
AS $$ SELECT nullif(current_setting('cihub.org_id', true), '') $$;

CREATE FUNCTION cihub_system() RETURNS boolean
LANGUAGE sql STABLE
AS $$ SELECT coalesce(current_setting('cihub.system', true), '') = 'on' $$;

CREATE FUNCTION cihub_tenant_table(tenant_table regclass) RETURNS void
LANGUAGE plpgsql
AS $$
DECLARE
    tenant_column text := CASE
        WHEN EXISTS (
            SELECT 1 FROM pg_attribute
            WHERE attrelid = tenant_table AND attname = 'org_id' AND NOT attisdropped
        ) THEN 'org_id'
        ELSE 'id'
    END;
BEGIN
    EXECUTE format('ALTER TABLE %s ENABLE ROW LEVEL SECURITY', tenant_table);
    EXECUTE format('ALTER TABLE %s FORCE ROW LEVEL SECURITY', tenant_table);
    EXECUTE format(
        'CREATE POLICY tenant ON %s
            USING (cihub_system() OR %I = cihub_org_id())
            WITH CHECK (cihub_system() OR %I = cihub_org_id())',
        tenant_table, tenant_column, tenant_column
    );
END
$$;

CREATE TYPE scm_kind AS ENUM ('github');
CREATE TYPE connection_kind AS ENUM ('github_app');
CREATE TYPE connection_mode AS ENUM ('normal', 'throttled', 'scm_unavailable', 'paused');
CREATE TYPE api_scope AS ENUM ('queues:read', 'queues:write', 'prs:write');

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

CREATE TABLE scm_connections (
    id                   text            PRIMARY KEY,
    org_id               text            NOT NULL REFERENCES orgs (id),
    scm_instance_id      text            NOT NULL,
    scm_account_id       text            NOT NULL,
    kind                 connection_kind NOT NULL,
    external_id          text            NOT NULL,
    suspended_at         timestamptz,
    mode                 connection_mode NOT NULL DEFAULT 'normal',
    mode_reason          text,
    rate_remaining       integer,
    rate_limit           integer,
    rate_reset_at        timestamptz,
    paused_until         timestamptz,
    consecutive_failures integer         NOT NULL DEFAULT 0,
    created_at           timestamptz     NOT NULL DEFAULT now(),
    UNIQUE (scm_instance_id, kind, external_id),
    UNIQUE (org_id, scm_instance_id, id),
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
    UNIQUE (org_id, id),
    FOREIGN KEY (org_id, scm_instance_id, connection_id)
        REFERENCES scm_connections (org_id, scm_instance_id, id)
);

CREATE INDEX repos_org_id ON repos (org_id);

CREATE TABLE users (
    id              text        PRIMARY KEY,
    scm_instance_id text        NOT NULL REFERENCES scm_instances (id),
    external_id     text        NOT NULL,
    login           text        NOT NULL,
    created_at      timestamptz NOT NULL DEFAULT now(),
    UNIQUE (scm_instance_id, external_id)
);

CREATE TABLE api_tokens (
    id           text        PRIMARY KEY,
    org_id       text        NOT NULL REFERENCES orgs (id),
    name         text        NOT NULL,
    token_hash   bytea       NOT NULL UNIQUE CHECK (length(token_hash) = 32),
    scopes       api_scope[] NOT NULL,
    created_at   timestamptz NOT NULL DEFAULT now(),
    expires_at   timestamptz,
    last_used_at timestamptz
);

CREATE INDEX api_tokens_org_id ON api_tokens (org_id);

CREATE TABLE audit_log (
    id             text        PRIMARY KEY,
    org_id         text        NOT NULL REFERENCES orgs (id),
    actor_user_id  text        REFERENCES users (id),
    actor_token_id text        REFERENCES api_tokens (id),
    action         text        NOT NULL,
    target         text        NOT NULL,
    before         jsonb,
    after          jsonb,
    occurred_at    timestamptz NOT NULL DEFAULT now()
);

CREATE INDEX audit_log_org_id_occurred_at ON audit_log (org_id, occurred_at);

SELECT cihub_tenant_table('orgs');
SELECT cihub_tenant_table('scm_accounts');
SELECT cihub_tenant_table('scm_connections');
SELECT cihub_tenant_table('repos');
SELECT cihub_tenant_table('api_tokens');
SELECT cihub_tenant_table('audit_log');
