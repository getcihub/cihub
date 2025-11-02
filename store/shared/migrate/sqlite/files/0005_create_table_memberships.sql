-- name: create-table-memberships

CREATE TABLE IF NOT EXISTS memberships (
  membership_installation_id INTEGER NOT NULL,
  membership_user_id         INTEGER NOT NULL,
  membership_role            TEXT NOT NULL,
  membership_state           TEXT NOT NULL,
  membership_synced          INTEGER,
  membership_created         INTEGER,
  membership_updated         INTEGER,
  PRIMARY KEY (membership_installation_id, membership_user_id),
  FOREIGN KEY (membership_installation_id) REFERENCES installations(installation_id),
  FOREIGN KEY (membership_user_id) REFERENCES users(user_id)
);

CREATE INDEX IF NOT EXISTS idx_memberships_user_id ON memberships(membership_user_id);
