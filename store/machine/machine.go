package machine

import (
	"context"
	"database/sql"

	"github.com/getcihub/cihub/core"
	"github.com/getcihub/cihub/store/shared/db"
)

type store struct {
	db *db.DB
}

// New returns a new MachineStore.
func New(db *db.DB) core.MachineStore {
	return &store{db}
}

// Create persists a new machine to the datastore.
func (s *store) Create(ctx context.Context, machine *core.Machine) error {
	return s.db.Lock(func(execer db.Execer, binder db.Binder) error {
		params := toParams(machine)
		stmt, args, err := binder.BindNamed(stmtInsert, params)
		if err != nil {
			return err
		}
		_, err = execer.Exec(stmt, args...)
		return err
	})
}

// Update persists an updated machine to the datastore.
func (s *store) Update(ctx context.Context, machine *core.Machine) error {
	return s.db.Lock(func(execer db.Execer, binder db.Binder) error {
		params := toParams(machine)
		stmt, args, err := binder.BindNamed(stmtUpdate, params)
		if err != nil {
			return err
		}
		_, err = execer.Exec(stmt, args...)
		return err
	})
}

// Delete deletes a machine from the datastore.
func (s *store) Delete(ctx context.Context, machine *core.Machine) error {
	return s.db.Lock(func(execer db.Execer, binder db.Binder) error {
		params := map[string]interface{}{"machine_name": machine.Name, "machine_owner": machine.Owner}
		stmt, args, err := binder.BindNamed(stmtDelete, params)
		if err != nil {
			return err
		}
		_, err = execer.Exec(stmt, args...)
		return err
	})
}

// Find returns a machine by hostname and owner, including associated runners.
func (s *store) Find(ctx context.Context, owner, name string) (*core.Machine, error) {
	var out []*core.Machine
	err := s.db.View(func(queryer db.Queryer, binder db.Binder) error {
		params := map[string]interface{}{
			"machine_name":       name,
			"machine_owner":      owner,
			"runner_status_idle": core.RunnerStatusIdle,
			"runner_status_busy": core.RunnerStatusBusy,
		}
		query, args, err := binder.BindNamed(queryFindWithRunners, params)
		if err != nil {
			return err
		}
		rows, err := queryer.Query(query, args...)
		if err != nil {
			return err
		}
		out, err = scanRowsWithRunners(rows)
		return err
	})
	if len(out) == 0 {
		return nil, sql.ErrNoRows
	}
	return out[0], err
}

// FindToken returns a machine by its authentication token, including associated runners.
func (s *store) FindToken(ctx context.Context, token string) (*core.Machine, error) {
	var out []*core.Machine
	err := s.db.View(func(queryer db.Queryer, binder db.Binder) error {
		params := map[string]interface{}{
			"machine_token":      token,
			"runner_status_idle": core.RunnerStatusIdle,
			"runner_status_busy": core.RunnerStatusBusy,
		}
		query, args, err := binder.BindNamed(queryFindTokenWithRunners, params)
		if err != nil {
			return err
		}
		rows, err := queryer.Query(query, args...)
		if err != nil {
			return err
		}
		out, err = scanRowsWithRunners(rows)
		return err
	})
	if len(out) == 0 {
		return nil, sql.ErrNoRows
	}
	return out[0], err
}

// List returns all machines owned by a user, including associated runners.
func (s *store) List(ctx context.Context, owner string) ([]*core.Machine, error) {
	var out []*core.Machine
	err := s.db.View(func(queryer db.Queryer, binder db.Binder) error {
		params := map[string]interface{}{
			"machine_owner":      owner,
			"runner_status_idle": core.RunnerStatusIdle,
			"runner_status_busy": core.RunnerStatusBusy,
		}
		query, args, err := binder.BindNamed(queryListWithRunners, params)
		if err != nil {
			return err
		}
		rows, err := queryer.Query(query, args...)
		if err != nil {
			return err
		}
		out, err = scanRowsWithRunners(rows)
		return err
	})
	return out, err
}

// Purge deletes offline machines (last_seen older than timestamp).
func (s *store) Purge(ctx context.Context, before int64) error {
	return s.db.Lock(func(execer db.Execer, binder db.Binder) error {
		params := map[string]interface{}{
			"machine_last_seen": before,
		}
		stmt, args, err := binder.BindNamed(stmtPurge, params)
		if err != nil {
			return err
		}
		_, err = execer.Exec(stmt, args...)
		return err
	})
}

const queryBaseWithRunners = `
SELECT
	machine_name,
	machine_owner,
	machine_arch,
	machine_cpu,
	machine_ram,
	machine_status,
	machine_created,
	machine_last_seen,
	machine_updated,
	machine_token,
	runner_name,
	runner_id,
	runner_installation_id,
	runner_owner,
	runner_status,
	runner_machine,
	runner_arch,
	runner_cpu,
	runner_ram,
	runner_image,
	runner_group_id,
	runner_labels,
	runner_cancelled,
	runner_created,
	runner_accepted,
	runner_started,
	runner_stopped,
	runner_updated,
	runner_token
`

const queryFindWithRunners = queryBaseWithRunners + `
FROM machines
LEFT JOIN runners ON machines.machine_name = runners.runner_machine
	AND machines.machine_owner = runners.runner_owner
	AND runners.runner_status IN (:runner_status_idle, :runner_status_busy)
WHERE machines.machine_name = :machine_name
AND machines.machine_owner = :machine_owner
ORDER BY machine_name, runner_name
`

const queryFindTokenWithRunners = queryBaseWithRunners + `
FROM machines
LEFT JOIN runners ON machines.machine_name = runners.runner_machine
	AND machines.machine_owner = runners.runner_owner
	AND runners.runner_status IN (:runner_status_idle, :runner_status_busy)
WHERE machines.machine_token = :machine_token
ORDER BY machine_name, runner_name
`

const queryListWithRunners = queryBaseWithRunners + `
FROM machines
LEFT JOIN runners ON machines.machine_name = runners.runner_machine
	AND machines.machine_owner = runners.runner_owner
	AND runners.runner_status IN (:runner_status_idle, :runner_status_busy)
WHERE machines.machine_owner = :machine_owner
ORDER BY machine_name, runner_name
`

const stmtDelete = `
DELETE FROM machines
WHERE machine_name = :machine_name
  AND machine_owner = :machine_owner
`

const stmtInsert = `
INSERT INTO machines (
	machine_name,
	machine_owner,
	machine_arch,
	machine_cpu,
	machine_ram,
	machine_status,
	machine_created,
	machine_last_seen,
	machine_updated,
	machine_token
) VALUES (
	:machine_name,
	:machine_owner,
	:machine_arch,
	:machine_cpu,
	:machine_ram,
	:machine_status,
	:machine_created,
	:machine_last_seen,
	:machine_updated,
	:machine_token
)
`

const stmtPurge = `
DELETE FROM machines
WHERE machine_last_seen > 0
AND machine_last_seen < :machine_last_seen
`

const stmtUpdate = `
UPDATE machines
SET
	machine_owner     = :machine_owner,
	machine_arch      = :machine_arch,
	machine_cpu       = :machine_cpu,
	machine_ram       = :machine_ram,
	machine_status    = :machine_status,
	machine_created   = :machine_created,
	machine_last_seen = :machine_last_seen,
	machine_updated   = :machine_updated,
	machine_token     = :machine_token
WHERE machine_name = :machine_name
  AND machine_owner = :machine_owner
`
