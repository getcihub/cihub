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
	{
		name: "create-table-installations",
		stmt: createTableInstallations,
	},
	{
		name: "create-table-memberships",
		stmt: createTableMemberships,
	},
	{
		name: "create-index-memberships-user",
		stmt: createIndexMembershipsUser,
	},
	{
		name: "create-index-memberships-installation",
		stmt: createIndexMembershipsInstallation,
	},
	{
		name: "create-table-machines",
		stmt: createTableMachines,
	},
	{
		name: "create-index-machines-owner",
		stmt: createIndexMachinesOwner,
	},
	{
		name: "create-index-machines-status",
		stmt: createIndexMachinesStatus,
	},
	{
		name: "create-index-machines-last-seen",
		stmt: createIndexMachinesLastSeen,
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
  user_synced         INTEGER,
  user_syncing        BOOLEAN,
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
  job_url             TEXT,
  job_author_login    TEXT,
  job_author_avatar   TEXT,
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
  runner_owner           TEXT,
  runner_status          TEXT,
  runner_machine         TEXT,
  runner_arch            TEXT,
  runner_cpu             INTEGER,
  runner_ram             INTEGER,
  runner_image           TEXT,
  runner_group_id        INTEGER,
  runner_labels          TEXT,
  runner_cancelled       INTEGER,
  runner_created         INTEGER,
  runner_accepted        INTEGER,
  runner_started         INTEGER,
  runner_stopped         INTEGER,
  runner_updated         INTEGER,
  runner_token           TEXT
);

CREATE INDEX IF NOT EXISTS ix_runner_status ON runners (runner_status);
CREATE INDEX IF NOT EXISTS ix_runner_created ON runners (runner_created);
CREATE INDEX IF NOT EXISTS ix_runner_machine ON runners (runner_machine);
`

//
// 0004_create_table_installations.sql
//

var createTableInstallations = `
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
`

//
// 0005_create_table_memberships.sql
//

var createTableMemberships = `
CREATE TABLE IF NOT EXISTS memberships (
  membership_installation_id INTEGER NOT NULL,
  membership_user_id         INTEGER NOT NULL,
  membership_role            TEXT NOT NULL,
  membership_state           TEXT NOT NULL,
  membership_synced          INTEGER,
  membership_created         INTEGER,
  membership_updated         INTEGER,
  PRIMARY KEY (membership_installation_id, membership_user_id),
  FOREIGN KEY (membership_installation_id) REFERENCES installations(installation_id) ON DELETE CASCADE,
  FOREIGN KEY (membership_user_id) REFERENCES users(user_id) ON DELETE CASCADE
);
`

var createIndexMembershipsUser = `
CREATE INDEX IF NOT EXISTS idx_memberships_user_id ON memberships(membership_user_id);
`

var createIndexMembershipsInstallation = `
CREATE INDEX IF NOT EXISTS idx_memberships_installation_id ON memberships(membership_installation_id);
`

//
// 0006_create_table_machines.sql
//

var createTableMachines = `
CREATE TABLE IF NOT EXISTS machines (
  machine_name           TEXT,
  machine_owner          TEXT,
  machine_arch           TEXT,
  machine_cpu            INTEGER,
  machine_cpu_limit      INTEGER,
  machine_cpu_allocated  INTEGER,
  machine_ram_total      INTEGER,
  machine_ram_available  INTEGER,
  machine_ram_limit      INTEGER,
  machine_ram_allocated  INTEGER,
  machine_status         TEXT,
  machine_created        INTEGER,
  machine_last_seen      INTEGER,
  machine_updated        INTEGER,
  machine_token          TEXT,

  PRIMARY KEY(machine_name, machine_owner)
  UNIQUE(machine_token)
);
`

var createIndexMachinesOwner = `
CREATE INDEX IF NOT EXISTS ix_machine_owner ON machines (machine_owner);
`

var createIndexMachinesStatus = `
CREATE INDEX IF NOT EXISTS ix_machine_status ON machines (machine_status);
`

var createIndexMachinesLastSeen = `
CREATE INDEX IF NOT EXISTS ix_machine_last_seen ON machines (machine_last_seen);
`
