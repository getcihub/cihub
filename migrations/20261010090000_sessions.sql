CREATE TYPE membership_role AS ENUM ('admin', 'member');

-- The role of a user in an organization, as the SCM gives it. The sign-in
-- and the refresh of each hour write it; a row not seen by a refresh is
-- deleted.
CREATE TABLE memberships (
    org_id       text            NOT NULL REFERENCES orgs (id),
    user_id      text            NOT NULL REFERENCES users (id),
    role         membership_role NOT NULL,
    refreshed_at timestamptz     NOT NULL,
    PRIMARY KEY (org_id, user_id)
);

CREATE INDEX memberships_user_id ON memberships (user_id);

SELECT cihub_tenant_table('memberships');

-- A session covers every organization of its user, so it is global, as
-- `users`; the organization of a request is checked through `memberships`.
CREATE TABLE sessions (
    id         text        PRIMARY KEY,
    user_id    text        NOT NULL REFERENCES users (id),
    token_hash bytea       NOT NULL UNIQUE CHECK (length(token_hash) = 32),
    created_at timestamptz NOT NULL,
    expires_at timestamptz NOT NULL
);

CREATE INDEX sessions_expires_at ON sessions (expires_at);
