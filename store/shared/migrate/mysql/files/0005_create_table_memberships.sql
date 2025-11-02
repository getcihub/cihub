-- name: create-table-memberships

CREATE TABLE IF NOT EXISTS memberships (
  membership_installation_id BIGINT NOT NULL,
  membership_user_id         BIGINT NOT NULL,
  membership_role            VARCHAR(50) NOT NULL,
  membership_state           VARCHAR(50) NOT NULL,
  membership_synced          BIGINT,
  membership_created         BIGINT,
  membership_updated         BIGINT,
  PRIMARY KEY (membership_installation_id, membership_user_id),
  FOREIGN KEY (membership_installation_id) REFERENCES installations(installation_id),
  FOREIGN KEY (membership_user_id) REFERENCES users(user_id)
);

CREATE INDEX idx_memberships_user_id ON memberships(membership_user_id);
