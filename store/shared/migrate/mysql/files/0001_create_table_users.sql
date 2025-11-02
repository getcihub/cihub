-- name: create-table-users

CREATE TABLE IF NOT EXISTS users (
  user_id             BIGINT PRIMARY KEY AUTO_INCREMENT,
  user_login          VARCHAR(255),
  user_email          VARCHAR(500),
  user_admin          BOOLEAN,
  user_active         BOOLEAN,
  user_avatar         VARCHAR(1000),
  user_created        BIGINT,
  user_updated        BIGINT,
  user_synced         BIGINT,
  user_syncing        BOOLEAN,
  user_oauth_token    TEXT,
  user_oauth_refresh  TEXT,
  user_oauth_expiry   BIGINT,
  user_token          VARCHAR(255),
  UNIQUE(user_login),
  UNIQUE(user_token)
);
