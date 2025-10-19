package sqlite

import (
	"database/sql"
)

var migrations = []struct {
	name string
	stmt string
}{
	{
		name: "create-table-users",
		stmt: createTableUsers,
	},
	{
		name: "create-table-jobs",
		stmt: createTableJobs,
	},
	{
		name: "create-table-runners",
		stmt: createTableRunners,
	},
}

// Migrate performs the database migration. If the migration fails
// and error is returned.
func Migrate(db *sql.DB) error {
	if err := createTable(db); err != nil {
		return err
	}
	completed, err := selectCompleted(db)
	if err != nil && err != sql.ErrNoRows {
		return err
	}
	for _, migration := range migrations {
		if _, ok := completed[migration.name]; ok {

			continue
		}

		if _, err := db.Exec(migration.stmt); err != nil {
			return err
		}
		if err := insertMigration(db, migration.name); err != nil {
			return err
		}

	}
	return nil
}

func createTable(db *sql.DB) error {
	_, err := db.Exec(migrationTableCreate)
	return err
}

func insertMigration(db *sql.DB, name string) error {
	_, err := db.Exec(migrationInsert, name)
	return err
}

func selectCompleted(db *sql.DB) (map[string]struct{}, error) {
	migrations := map[string]struct{}{}
	rows, err := db.Query(migrationSelect)
	if err != nil {
		return nil, err
	}
	defer rows.Close()
	for rows.Next() {
		var name string
		if err := rows.Scan(&name); err != nil {
			return nil, err
		}
		migrations[name] = struct{}{}
	}
	return migrations, nil
}

//
// migration table ddl and sql
//

var migrationTableCreate = `
CREATE TABLE IF NOT EXISTS migrations (
 name VARCHAR(255)
,UNIQUE(name)
)
`

var migrationInsert = `
INSERT INTO migrations (name) VALUES (?)
`

var migrationSelect = `
SELECT name FROM migrations
`

//
// 0001_create_table_users.sql
//

var createTableUsers = `
CREATE TABLE IF NOT EXISTS users (
  user_id             INTEGER PRIMARY KEY AUTOINCREMENT,
  user_login          TEXT COLLATE NOCASE,
  user_email          TEXT,
  user_admin          BOOLEAN,
  user_active         BOOLEAN,
  user_avatar         TEXT,
  user_created        INTEGER,
  user_updated        INTEGER,
  user_oauth_token    TEXT,
  user_oauth_refresh  TEXT,
  user_oauth_expiry   INTEGER,
  user_token          TEXT,
  UNIQUE(user_login COLLATE NOCASE),
  UNIQUE(user_token)
);
`

//
// 0002_create_table_jobs.sql
//

var createTableJobs = `
CREATE TABLE IF NOT EXISTS jobs (
  job_id              INTEGER PRIMARY KEY,
  job_run_id          INTEGER,
  job_installation_id INTEGER,
  job_owner           TEXT,
  job_repo            TEXT,
  job_workflow        TEXT,
  job_name            TEXT,
  job_branch          TEXT,
  job_sha             TEXT,
  job_status          TEXT,
  job_conclusion      TEXT,
  job_labels          TEXT,
  job_runner_id       INTEGER,
  job_runner_name     TEXT,
  job_machine         TEXT,
  job_url             TEXT,
  job_accepted        INTEGER,
  job_queued          INTEGER,
  job_started         INTEGER,
  job_completed       INTEGER,
  job_created         INTEGER,
  job_updated         INTEGER,
  job_version         INTEGER
);

CREATE INDEX IF NOT EXISTS ix_job_run_id ON jobs (job_run_id);
CREATE INDEX IF NOT EXISTS ix_job_status ON jobs (job_status);
CREATE INDEX IF NOT EXISTS ix_job_runner_id ON jobs (job_runner_id);
CREATE INDEX IF NOT EXISTS ix_job_machine ON jobs (job_machine);
CREATE INDEX IF NOT EXISTS ix_job_created ON jobs (job_created);
`

//
// 0003_create_table_runners.sql
//

var createTableRunners = `
CREATE TABLE IF NOT EXISTS runners (
  runner_name            TEXT PRIMARY KEY COLLATE NOCASE,
  runner_id              INTEGER,
  runner_installation_id INTEGER,
  runner_status          TEXT,
  runner_assigned_to     INTEGER,
  runner_cancelled       BOOLEAN,
  runner_completed       INTEGER,
  runner_created         INTEGER,
  runner_started         INTEGER,
  runner_stopped         INTEGER,
  runner_updated         INTEGER,
  runner_timeout         INTEGER,
  runner_token           TEXT
);

CREATE INDEX IF NOT EXISTS ix_runner_status ON runners (runner_status);
CREATE INDEX IF NOT EXISTS ix_runner_assigned_to ON runners (runner_assigned_to);
CREATE INDEX IF NOT EXISTS ix_runner_created ON runners (runner_created);
`
