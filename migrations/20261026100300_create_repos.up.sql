CREATE TABLE repos (
    id             text        PRIMARY KEY,
    org_id         text        NOT NULL REFERENCES orgs (id),
    connection_id  text        NOT NULL,
    external_id    text        NOT NULL,
    owner          text        NOT NULL,
    name           text        NOT NULL,
    default_branch text        NOT NULL,
    removed_at     timestamptz,
    created_at     timestamptz NOT NULL DEFAULT now(),
    updated_at     timestamptz NOT NULL DEFAULT now(),
    UNIQUE (org_id, id),
    UNIQUE (connection_id, external_id),
    FOREIGN KEY (org_id, connection_id) REFERENCES scm_connections (org_id, id)
);

CREATE INDEX repos_org_id ON repos (org_id);

SELECT cihub_tenant_table('repos');
