package postgres

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
INSERT INTO migrations (name) VALUES ($1)
`

var migrationSelect = `
SELECT name FROM migrations
`

//
// 0001_create_table_users.sql
//

var createTableUsers = `
CREATE TABLE IF NOT EXISTS users (
  user_id             BIGSERIAL PRIMARY KEY,
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
  job_url             VARCHAR(1000),
  job_author_login    VARCHAR(255),
  job_author_avatar   VARCHAR(1000),
  job_queued          BIGINT,
  job_started         BIGINT,
  job_completed       BIGINT,
  job_created         BIGINT,
  job_updated         BIGINT,
  job_version         BIGINT
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
  runner_name            VARCHAR(255) PRIMARY KEY,
  runner_id              BIGINT,
  runner_installation_id BIGINT,
  runner_owner           VARCHAR(255),
  runner_status          VARCHAR(50),
  runner_machine         VARCHAR(255),
  runner_arch            VARCHAR(50),
  runner_cpu             BIGINT,
  runner_ram             BIGINT,
  runner_image           VARCHAR(255),
  runner_group_id        BIGINT,
  runner_labels          TEXT,
  runner_cancelled       BIGINT,
  runner_created         BIGINT,
  runner_accepted        BIGINT,
  runner_started         BIGINT,
  runner_stopped         BIGINT,
  runner_updated         BIGINT,
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
  installation_id           BIGINT PRIMARY KEY,
  installation_login        VARCHAR(255) NOT NULL,
  installation_avatar       VARCHAR(1000),
  installation_type         VARCHAR(50) NOT NULL,
  installation_created      BIGINT,
  installation_suspended    BIGINT,
  installation_updated      BIGINT,
  UNIQUE(installation_login)
);
`

//
// 0005_create_table_memberships.sql
//

var createTableMemberships = `
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

CREATE INDEX IF NOT EXISTS idx_memberships_user_id ON memberships(membership_user_id);
`

//
// 0006_create_table_machines.sql
//

var createTableMachines = `
CREATE TABLE IF NOT EXISTS machines (
  machine_name            VARCHAR(255),
  machine_owner           VARCHAR(255),
  machine_arch            VARCHAR(50),
  machine_cpu             BIGINT,
  machine_cpu_limit       BIGINT,
  machine_cpu_allocated   BIGINT,
  machine_ram_total       BIGINT,
  machine_ram_available   BIGINT,
  machine_ram_limit       BIGINT,
  machine_ram_allocated   BIGINT,
  machine_status          VARCHAR(50),
  machine_created         BIGINT,
  machine_last_seen       BIGINT,
  machine_updated         BIGINT,
  machine_token           TEXT,

  PRIMARY KEY(machine_name, machine_owner),
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
