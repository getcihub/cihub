-- name: create-table-installations

CREATE TABLE IF NOT EXISTS installations (
  installation_id           INTEGER PRIMARY KEY,
  installation_login        TEXT COLLATE NOCASE NOT NULL,
  installation_avatar       TEXT,
  installation_type         TEXT NOT NULL,
  installation_created      INTEGER,
  installation_suspended    INTEGER,
  installation_updated      INTEGER,

  UNIQUE(installation_login COLLATE NOCASE)
);
