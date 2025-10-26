package job

import (
	"context"

	"github.com/getcihub/cihub/core"
	"github.com/getcihub/cihub/logger"
	"github.com/getcihub/cihub/store/shared/db"
)

type store struct {
	db *db.DB
}

// New returns a new JobStore.
func New(db *db.DB) core.JobStore {
	return &store{db}
}

func (s *store) Count(context.Context) (int64, error) {
	var out int64
	err := s.db.View(func(queryer db.Queryer, binder db.Binder) error {
		return queryer.QueryRow(queryCount).Scan(&out)
	})
	return out, err
}

func (s *store) Create(ctx context.Context, job *core.Job) error {
	return s.db.Lock(func(execer db.Execer, binder db.Binder) error {
		params := toParams(job)
		stmt, args, err := binder.BindNamed(stmtInsert, params)
		if err != nil {
			return err
		}
		_, err = execer.Exec(stmt, args...)
		return err
	})
}

// Delete deletes a job from the datastore.
func (s *store) Delete(ctx context.Context, job *core.Job) error {
	return s.db.Lock(func(execer db.Execer, binder db.Binder) error {
		params := map[string]any{"job_id": job.ID}
		stmt, args, err := binder.BindNamed(stmtDelete, params)
		if err != nil {
			return err
		}
		_, err = execer.Exec(stmt, args...)
		return err
	})
}

// Find returns a job from the datastore by its ID.
func (s *store) Find(ctx context.Context, id int64) (*core.Job, error) {
	out := &core.Job{ID: id}
	err := s.db.View(func(queryer db.Queryer, binder db.Binder) error {
		params := map[string]interface{}{"job_id": id}
		query, args, err := binder.BindNamed(queryFind, params)
		if err != nil {
			return err
		}
		row := queryer.QueryRow(query, args...)
		return scanRow(row, out)
	})
	return out, err
}

// FindRunID returns jobs from the datastore by workflow run ID.
func (s *store) FindRunID(ctx context.Context, runID int64) ([]*core.Job, error) {
	var out []*core.Job
	err := s.db.View(func(queryer db.Queryer, binder db.Binder) error {
		params := map[string]interface{}{"job_run_id": runID}
		query, args, err := binder.BindNamed(queryFindRunID, params)
		if err != nil {
			return err
		}
		rows, err := queryer.Query(query, args...)
		if err != nil {
			return err
		}
		defer func() {
			if err := rows.Close(); err != nil {
				logger.FromContext(ctx).
					WithError(err).
					Warnln("store: cannot close job rows")
			}
		}()
		out, err = scanRows(rows)
		return err
	})
	return out, err
}

// List returns a list of jobs from the datastore.
func (s *store) List(ctx context.Context, q core.JobParams) ([]*core.Job, error) {
	var out []*core.Job
	err := s.db.View(func(queryer db.Queryer, binder db.Binder) error {
		limit := q.Limit + 1

		query := queryList
		params := map[string]any{"limit": limit}
		if q.After != 0 {
			query = queryListOffset
			params["job_id"] = q.After
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
					Warnln("store: cannot close job rows")
			}
		}()

		out, err = scanRows(rows)
		return err
	})
	return out, err
}

// ListStatus returns all jobs filtered by status without pagination.
func (s *store) ListStatus(ctx context.Context, status string) ([]*core.Job, error) {
	var out []*core.Job
	err := s.db.View(func(queryer db.Queryer, binder db.Binder) error {
		params := map[string]any{"job_status": status}

		stmt, args, err := binder.BindNamed(queryListStatusAll, params)
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
					Warnln("store: cannot close job rows")
			}
		}()

		out, err = scanRows(rows)
		return err
	})
	return out, err
}

// Purge deletes all completed jobs older than the given unix timestamp.
func (s *store) Purge(ctx context.Context, before int64) error {
	return s.db.Lock(func(execer db.Execer, binder db.Binder) error {
		params := map[string]any{
			"job_status":    core.JobStatusCompleted,
			"job_completed": before,
		}
		stmt, args, err := binder.BindNamed(stmtPurge, params)
		if err != nil {
			return err
		}
		_, err = execer.Exec(stmt, args...)
		return err
	})
}

// Update persists an updated job to the datastore using optimistic locking.
func (s *store) Update(ctx context.Context, job *core.Job) error {
	versionNew := job.Version + 1
	versionOld := job.Version

	err := s.db.Lock(func(execer db.Execer, binder db.Binder) error {
		params := toParams(job)
		params["job_version_old"] = versionOld
		params["job_version_new"] = versionNew

		stmt, args, err := binder.BindNamed(stmtUpdate, params)
		if err != nil {
			return err
		}
		res, err := execer.Exec(stmt, args...)
		if err != nil {
			return err
		}
		effected, err := res.RowsAffected()
		if err != nil {
			return err
		}
		if effected == 0 {
			return db.ErrOptimisticLock
		}
		return nil
	})
	if err == nil {
		job.Version = versionNew
	}
	return err
}

const queryBase = `
SELECT
	job_id,
	job_run_id,
	job_installation_id,
	job_owner,
	job_repo,
	job_workflow,
	job_name,
	job_branch,
	job_sha,
	job_status,
	job_conclusion,
	job_labels,
	job_runner_id,
	job_runner_name,
	job_machine,
	job_url,
	job_os,
	job_arch,
	job_memory,
	job_vcpu,
	job_accepted,
	job_queued,
	job_started,
	job_completed,
	job_created,
	job_updated,
	job_version
`

const queryCount = `
SELECT COUNT(*) FROM jobs
`

const queryFind = queryBase + `
FROM jobs
WHERE job_id = :job_id
`

const queryFindRunID = queryBase + `
FROM jobs
WHERE job_run_id = :job_run_id
ORDER BY job_created
`

const queryList = queryBase + `
FROM jobs
ORDER BY job_id
LIMIT :limit
`

const queryListOffset = queryBase + `
FROM jobs
WHERE job_id > :job_id
ORDER BY job_id
LIMIT :limit
`

const queryListStatusAll = queryBase + `
FROM jobs
WHERE job_status = :job_status
ORDER BY job_id
`

const stmtDelete = `
DELETE FROM jobs WHERE job_id = :job_id
`

const stmtInsert = `
INSERT INTO jobs (
	job_id,
	job_run_id,
	job_installation_id,
	job_owner,
	job_repo,
	job_workflow,
	job_name,
	job_branch,
	job_sha,
	job_status,
	job_conclusion,
	job_labels,
	job_runner_id,
	job_runner_name,
	job_machine,
	job_url,
	job_os,
	job_arch,
	job_memory,
	job_vcpu,
	job_accepted,
	job_queued,
	job_started,
	job_completed,
	job_created,
	job_updated,
	job_version
) VALUES (
	:job_id,
	:job_run_id,
	:job_installation_id,
	:job_owner,
	:job_repo,
	:job_workflow,
	:job_name,
	:job_branch,
	:job_sha,
	:job_status,
	:job_conclusion,
	:job_labels,
	:job_runner_id,
	:job_runner_name,
	:job_machine,
	:job_url,
	:job_os,
	:job_arch,
	:job_memory,
	:job_vcpu,
	:job_accepted,
	:job_queued,
	:job_started,
	:job_completed,
	:job_created,
	:job_updated,
	:job_version
)
`

const stmtPurge = `
DELETE FROM jobs
WHERE job_status = :job_status
AND job_completed > 0
AND job_completed < :job_completed
`

const stmtUpdate = `
UPDATE jobs
SET
	job_run_id          = :job_run_id,
	job_installation_id = :job_installation_id,
	job_owner           = :job_owner,
	job_repo            = :job_repo,
	job_workflow        = :job_workflow,
	job_name            = :job_name,
	job_branch          = :job_branch,
	job_sha             = :job_sha,
	job_status          = :job_status,
	job_conclusion      = :job_conclusion,
	job_labels          = :job_labels,
	job_runner_id       = :job_runner_id,
	job_runner_name     = :job_runner_name,
	job_machine         = :job_machine,
	job_url             = :job_url,
	job_os              = :job_os,
	job_arch            = :job_arch,
	job_memory          = :job_memory,
	job_vcpu            = :job_vcpu,
	job_accepted        = :job_accepted,
	job_queued          = :job_queued,
	job_started         = :job_started,
	job_completed       = :job_completed,
	job_updated         = :job_updated,
	job_version         = :job_version_new
WHERE job_id = :job_id
AND job_version = :job_version_old
`
