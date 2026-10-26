CREATE TABLE users (
    id                           text        PRIMARY KEY,
    external_id                  text        NOT NULL UNIQUE,
    login                        text        NOT NULL,
    name                         text,
    avatar_url                   text,
    scm_token                    bytea,
    scm_token_expires_at         timestamptz,
    scm_refresh_token            bytea,
    scm_refresh_token_expires_at timestamptz,
    synced_at                    timestamptz,
    created_at                   timestamptz NOT NULL DEFAULT now(),
    updated_at                   timestamptz NOT NULL DEFAULT now()
);

CREATE INDEX users_login ON users (lower(login));
