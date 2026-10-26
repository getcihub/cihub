CREATE TYPE connection_kind AS ENUM ('github_app');
CREATE TYPE connection_mode AS ENUM ('normal', 'throttled', 'scm_unavailable', 'paused');

CREATE TABLE scm_connections (
    id                   text            PRIMARY KEY,
    org_id               text            NOT NULL REFERENCES orgs (id),
    account_external_id  text            NOT NULL,
    kind                 connection_kind NOT NULL,
    external_id          text            NOT NULL,
    suspended_at         timestamptz,
    synced_at            timestamptz     NOT NULL,
    permissions          jsonb           NOT NULL,
    mode                 connection_mode NOT NULL DEFAULT 'normal',
    mode_reason          text,
    rate_remaining       integer,
    rate_limit           integer,
    rate_reset_at        timestamptz,
    paused_until         timestamptz,
    consecutive_failures integer         NOT NULL DEFAULT 0,
    created_at           timestamptz     NOT NULL DEFAULT now(),
    UNIQUE (kind, external_id),
    UNIQUE (account_external_id),
    UNIQUE (org_id, id)
);

CREATE INDEX scm_connections_org_id ON scm_connections (org_id);

SELECT cihub_tenant_table('scm_connections');
