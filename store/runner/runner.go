package runner

import (
	"context"

	"github.com/getcihub/cihub/core"
	"github.com/getcihub/cihub/logger"
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

func (s *store) Count(context.Context) (int64, error) {
	var out int64
	err := s.db.View(func(queryer db.Queryer, binder db.Binder) error {
		return queryer.QueryRow(queryCount).Scan(&out)
	})
	return out, err
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

// FindAssignedTo returns a runner assigned to a specific job ID.
func (s *store) FindAssignedTo(ctx context.Context, jobID int64) (*core.Runner, error) {
	out := &core.Runner{AssignedTo: jobID}
	err := s.db.View(func(queryer db.Queryer, binder db.Binder) error {
		params := map[string]interface{}{"runner_assigned_to": jobID}
		query, args, err := binder.BindNamed(queryFindAssignedTo, params)
		if err != nil {
			return err
		}
		row := queryer.QueryRow(query, args...)
		return scanRow(s.enc, row, out)
	})
	return out, err
}

// List returns a list of runners from the datastore.
func (s *store) List(ctx context.Context, q core.RunnerParams) ([]*core.Runner, error) {
	var out []*core.Runner
	err := s.db.View(func(queryer db.Queryer, binder db.Binder) error {
		limit := q.Limit + 1

		query := queryList
		params := map[string]any{"limit": limit}
		if q.After != "" {
			query = queryListOffset
			params["runner_name"] = q.After
		}

		stmt, args, err := binder.BindNamed(query, params)
		if err != nil {
			return err
		}

		rows, err := queryer.Query(stmt, args...)
		if err != nil {
			return err
		}

		defer func() {
			if err := rows.Close(); err != nil {
				logger.FromContext(ctx).
					WithError(err).
					Warnln("store: cannot close runner rows")
			}
		}()

		out, err = scanRows(s.enc, rows)
		return err
	})
	return out, err
}

// ListStatus returns a list of runners filtered by status.
func (s *store) ListStatus(ctx context.Context, status string, q core.RunnerParams) ([]*core.Runner, error) {
	var out []*core.Runner
	err := s.db.View(func(queryer db.Queryer, binder db.Binder) error {
		limit := q.Limit + 1

		query := queryListStatus
		params := map[string]any{
			"limit":         limit,
			"runner_status": status,
		}
		if q.After != "" {
			query = queryListStatusOffset
			params["runner_name"] = q.After
		}

		stmt, args, err := binder.BindNamed(query, params)
		if err != nil {
			return err
		}

		rows, err := queryer.Query(stmt, args...)
		if err != nil {
			return err
		}

		defer func() {
			if err := rows.Close(); err != nil {
				logger.FromContext(ctx).
					WithError(err).
					Warnln("store: cannot close runner rows")
			}
		}()

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
	runner_repo,
	runner_status,
	runner_assigned_to,
	runner_cancelled,
	runner_completed,
	runner_created,
	runner_started,
	runner_stopped,
	runner_updated,
	runner_timeout,
	runner_token
`

const queryCount = `
SELECT COUNT(*) FROM runners
`

const queryFind = queryBase + `
FROM runners
WHERE runner_name = :runner_name
`

const queryFindID = queryBase + `
FROM runners
WHERE runner_id = :runner_id
`

const queryFindAssignedTo = queryBase + `
FROM runners
WHERE runner_assigned_to = :runner_assigned_to
`

const queryList = queryBase + `
FROM runners
ORDER BY runner_name
LIMIT :limit
`

const queryListOffset = queryBase + `
FROM runners
WHERE runner_name > :runner_name
ORDER BY runner_name
LIMIT :limit
`

const queryListStatus = queryBase + `
FROM runners
WHERE runner_status = :runner_status
ORDER BY runner_name
LIMIT :limit
`

const queryListStatusOffset = queryBase + `
FROM runners
WHERE runner_status = :runner_status
AND runner_name > :runner_name
ORDER BY runner_name
LIMIT :limit
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
	runner_repo,
	runner_status,
	runner_assigned_to,
	runner_cancelled,
	runner_completed,
	runner_created,
	runner_started,
	runner_stopped,
	runner_updated,
	runner_timeout,
	runner_token
) VALUES (
	:runner_name,
	:runner_id,
	:runner_installation_id,
	:runner_owner,
	:runner_repo,
	:runner_status,
	:runner_assigned_to,
	:runner_cancelled,
	:runner_completed,
	:runner_created,
	:runner_started,
	:runner_stopped,
	:runner_updated,
	:runner_timeout,
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
	runner_repo            = :runner_repo,
	runner_status          = :runner_status,
	runner_assigned_to     = :runner_assigned_to,
	runner_cancelled       = :runner_cancelled,
	runner_completed       = :runner_completed,
	runner_started         = :runner_started,
	runner_stopped         = :runner_stopped,
	runner_updated         = :runner_updated,
	runner_timeout         = :runner_timeout,
	runner_token           = :runner_token
WHERE runner_name = :runner_name
`
