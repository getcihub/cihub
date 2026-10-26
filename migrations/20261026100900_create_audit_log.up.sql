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

SELECT cihub_tenant_table('audit_log');
