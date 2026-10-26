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
