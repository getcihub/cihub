package mysql

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
  user_id             BIGINT PRIMARY KEY AUTO_INCREMENT,
  user_login          VARCHAR(255),
  user_email          VARCHAR(500),
  user_admin          BOOLEAN,
  user_active         BOOLEAN,
  user_avatar         VARCHAR(1000),
  user_created        BIGINT,
  user_updated        BIGINT,
  user_oauth_token    TEXT,
  user_oauth_refresh  TEXT,
  user_oauth_expiry   BIGINT,
  user_token          VARCHAR(255),
  UNIQUE(user_login),
  UNIQUE(user_token)
);
`

//
// 0002_create_table_jobs.sql
//

var createTableJobs = `
CREATE TABLE IF NOT EXISTS jobs (
  job_id              BIGINT PRIMARY KEY,
  job_run_id          BIGINT,
  job_installation_id BIGINT,
  job_owner           VARCHAR(255),
  job_repo            VARCHAR(255),
  job_workflow        VARCHAR(500),
  job_name            VARCHAR(500),
  job_branch          VARCHAR(255),
  job_sha             VARCHAR(255),
  job_status          VARCHAR(50),
  job_conclusion      VARCHAR(50),
  job_labels          TEXT,
  job_runner_id       BIGINT,
  job_runner_name     VARCHAR(255),
  job_machine         VARCHAR(255),
  job_url             VARCHAR(1000),
  job_accepted        BIGINT,
  job_queued          BIGINT,
  job_started         BIGINT,
  job_completed       BIGINT,
  job_created         BIGINT,
  job_updated         BIGINT,
  job_version         BIGINT
);

CREATE INDEX ix_job_run_id ON jobs (job_run_id);
CREATE INDEX ix_job_status ON jobs (job_status);
CREATE INDEX ix_job_runner_id ON jobs (job_runner_id);
CREATE INDEX ix_job_machine ON jobs (job_machine);
CREATE INDEX ix_job_created ON jobs (job_created);
`

//
// 0003_create_table_runners.sql
//

var createTableRunners = `
CREATE TABLE IF NOT EXISTS runners (
  runner_name            VARCHAR(255) PRIMARY KEY,
  runner_id              BIGINT,
  runner_installation_id BIGINT,
  runner_owner           VARCHAR(255),
  runner_status          VARCHAR(50),
  runner_assigned_to     BIGINT,
  runner_cancelled       BOOLEAN,
  runner_completed       BIGINT,
  runner_created         BIGINT,
  runner_started         BIGINT,
  runner_stopped         BIGINT,
  runner_updated         BIGINT,
  runner_timeout         BIGINT,
  runner_token           TEXT
);

CREATE INDEX ix_runner_status ON runners (runner_status);
CREATE INDEX ix_runner_assigned_to ON runners (runner_assigned_to);
CREATE INDEX ix_runner_created ON runners (runner_created);
`
