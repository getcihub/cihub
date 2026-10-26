CREATE TABLE orgs (
    id          text        PRIMARY KEY,
    slug        text        NOT NULL UNIQUE,
    name        text,
    description text,
    avatar_url  text,
    created_at  timestamptz NOT NULL DEFAULT now(),
    updated_at  timestamptz NOT NULL DEFAULT now()
);

SELECT cihub_tenant_table('orgs');
