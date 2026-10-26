CREATE TYPE api_scope AS ENUM ('queues:read', 'queues:write', 'prs:write');

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

SELECT cihub_tenant_table('api_tokens');
