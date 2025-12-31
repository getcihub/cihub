package runner

import (
	"context"

	"github.com/getcihub/cihub/core"
	"github.com/getcihub/cihub/store/shared/db"
	"github.com/getcihub/cihub/store/shared/encrypter"
)

type store struct {
	db  *db.DB
	enc encrypter.Encrypter
}

// New returns a new RunnerStore.
func New(db *db.DB, enc encrypter.Encrypter) core.RunnerStore {
	return &store{db, enc}
}

func (s *store) Create(ctx context.Context, runner *core.Runner) error {
	return s.db.Lock(func(execer db.Execer, binder db.Binder) error {
		params, err := toParams(s.enc, runner)
		if err != nil {
			return err
		}
		stmt, args, err := binder.BindNamed(stmtInsert, params)
		if err != nil {
			return err
		}
		_, err = execer.Exec(stmt, args...)
		return err
	})
}

// Delete deletes a runner from the datastore.
func (s *store) Delete(ctx context.Context, runner *core.Runner) error {
	return s.db.Lock(func(execer db.Execer, binder db.Binder) error {
		params := map[string]any{"runner_name": runner.Name}
		stmt, args, err := binder.BindNamed(stmtDelete, params)
		if err != nil {
			return err
		}
		_, err = execer.Exec(stmt, args...)
		return err
	})
}

// Find returns a runner from the datastore by its name.
func (s *store) Find(ctx context.Context, name string) (*core.Runner, error) {
	out := &core.Runner{Name: name}
	err := s.db.View(func(queryer db.Queryer, binder db.Binder) error {
		params := map[string]interface{}{"runner_name": name}
		query, args, err := binder.BindNamed(queryFind, params)
		if err != nil {
			return err
		}
		row := queryer.QueryRow(query, args...)
		return scanRow(s.enc, row, out)
	})
	return out, err
}

// FindID returns a runner from the datastore by its GitHub runner ID.
func (s *store) FindID(ctx context.Context, id int64) (*core.Runner, error) {
	out := &core.Runner{ID: id}
	err := s.db.View(func(queryer db.Queryer, binder db.Binder) error {
		params := map[string]interface{}{"runner_id": id}
		query, args, err := binder.BindNamed(queryFindID, params)
		if err != nil {
			return err
		}
		row := queryer.QueryRow(query, args...)
		return scanRow(s.enc, row, out)
	})
	return out, err
}

// ListStatus returns a slice of runners by status.
func (s *store) ListStatus(ctx context.Context, status core.RunnerStatus) ([]*core.Runner, error) {
	var out []*core.Runner
	err := s.db.View(func(queryer db.Queryer, binder db.Binder) error {
		params := map[string]interface{}{"runner_status": status}
		query, args, err := binder.BindNamed(queryListStatus, params)
		if err != nil {
			return err
		}
		rows, err := queryer.Query(query, args...)
		if err != nil {
			return err
		}
		out, err = scanRows(s.enc, rows)
		return err
	})
	return out, err
}

// ListMachine returns a slice of runners for a given machine.
func (s *store) ListMachine(ctx context.Context, machine *core.Machine) ([]*core.Runner, error) {
	var out []*core.Runner
	err := s.db.View(func(queryer db.Queryer, binder db.Binder) error {
		params := map[string]interface{}{"runner_owner": machine.Owner, "runner_machine": machine.Name}
		query, args, err := binder.BindNamed(queryListMachine, params)
		if err != nil {
			return err
		}
		rows, err := queryer.Query(query, args...)
		if err != nil {
			return err
		}
		out, err = scanRows(s.enc, rows)
		return err
	})
	return out, err
}

// Purge deletes all stopped runners older than the given unix timestamp.
func (s *store) Purge(ctx context.Context, before int64) error {
	return s.db.Lock(func(execer db.Execer, binder db.Binder) error {
		params := map[string]any{
			"runner_status":  core.RunnerStatusCompleted,
			"runner_stopped": before,
		}
		stmt, args, err := binder.BindNamed(stmtPurge, params)
		if err != nil {
			return err
		}
		_, err = execer.Exec(stmt, args...)
		return err
	})
}

// Update persists an updated runner to the datastore.
func (s *store) Update(ctx context.Context, runner *core.Runner) error {
	return s.db.Lock(func(execer db.Execer, binder db.Binder) error {
		params, err := toParams(s.enc, runner)
		if err != nil {
			return err
		}

		stmt, args, err := binder.BindNamed(stmtUpdate, params)
		if err != nil {
			return err
		}
		_, err = execer.Exec(stmt, args...)
		return err
	})
}

const queryBase = `
SELECT
	runner_name,
	runner_id,
	runner_installation_id,
	runner_owner,
	runner_status,
	runner_machine,
	runner_arch,
	runner_cpu,
	runner_ram,
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

const queryFind = queryBase + `
FROM runners
WHERE runner_name = :runner_name
`

const queryFindID = queryBase + `
FROM runners
WHERE runner_id = :runner_id
`

const queryListStatus = queryBase + `
FROM runners
WHERE runner_status = :runner_status
ORDER BY runner_name
`

const queryListMachine = queryBase + `
FROM runners
WHERE runner_owner = :runner_owner
  AND runner_machine = :runner_machine
  AND runner_status != 'completed'
`

const stmtDelete = `
DELETE FROM runners WHERE runner_name = :runner_name
`

const stmtInsert = `
INSERT INTO runners (
	runner_name,
	runner_id,
	runner_installation_id,
	runner_owner,
	runner_status,
	runner_machine,
	runner_arch,
	runner_cpu,
	runner_ram,
	runner_group_id,
	runner_labels,
	runner_cancelled,
	runner_created,
	runner_accepted,
	runner_started,
	runner_stopped,
	runner_updated,
	runner_token
) VALUES (
	:runner_name,
	:runner_id,
	:runner_installation_id,
	:runner_owner,
	:runner_status,
	:runner_machine,
	:runner_arch,
	:runner_cpu,
	:runner_ram,
	:runner_group_id,
	:runner_labels,
	:runner_cancelled,
	:runner_created,
	:runner_accepted,
	:runner_started,
	:runner_stopped,
	:runner_updated,
	:runner_token
)
`

const stmtPurge = `
DELETE FROM runners
WHERE runner_status = :runner_status
AND runner_stopped > 0
AND runner_stopped < :runner_stopped
`

const stmtUpdate = `
UPDATE runners
SET
	runner_id              = :runner_id,
	runner_installation_id = :runner_installation_id,
	runner_owner           = :runner_owner,
	runner_status          = :runner_status,
	runner_machine         = :runner_machine,
	runner_arch            = :runner_arch,
	runner_cpu             = :runner_cpu,
	runner_ram             = :runner_ram,
	runner_group_id        = :runner_group_id,
	runner_labels          = :runner_labels,
	runner_cancelled       = :runner_cancelled,
	runner_created         = :runner_created,
	runner_accepted        = :runner_accepted,
	runner_started         = :runner_started,
	runner_stopped         = :runner_stopped,
	runner_updated         = :runner_updated,
	runner_token           = :runner_token
WHERE runner_name = :runner_name
`
