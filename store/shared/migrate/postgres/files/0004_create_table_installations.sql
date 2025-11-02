-- name: create-table-installations

CREATE TABLE IF NOT EXISTS installations (
  installation_id           BIGINT PRIMARY KEY,
  installation_login        VARCHAR(255) NOT NULL,
  installation_avatar       VARCHAR(1000),
  installation_type         VARCHAR(50) NOT NULL,
  installation_created      BIGINT,
  installation_suspended    BIGINT,
  installation_updated      BIGINT,
  UNIQUE(installation_login)
);
