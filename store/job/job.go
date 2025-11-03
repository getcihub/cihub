package job

import (
	"context"

	"github.com/getcihub/cihub/core"
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
func (s *store) Find(ctx context.Context, owner string, id int64) (*core.Job, error) {
	out := &core.Job{ID: id, Owner: owner}
	err := s.db.View(func(queryer db.Queryer, binder db.Binder) error {
		params := map[string]interface{}{"job_id": id, "job_owner": owner}
		query, args, err := binder.BindNamed(queryFind, params)
		if err != nil {
			return err
		}
		row := queryer.QueryRow(query, args...)
		return scanRow(row, out)
	})
	return out, err
}

// ListIncomplete returns a list of jobs from the
// datastore with imcomplete status.
func (s *store) ListIncomplete(ctx context.Context, owner string) ([]*core.Job, error) {
	var out []*core.Job
	err := s.db.View(func(queryer db.Queryer, binder db.Binder) error {
		params := map[string]interface{}{"owner": owner}
		query, args, err := binder.BindNamed(queryJobIncomplete, params)
		if err != nil {
			return err
		}
		rows, err := queryer.Query(query, args...)
		if err != nil {
			return err
		}
		out, err = scanRows(rows)
		return err
	})
	return out, err
}

// ListCompleted returns a list of jobs from the datastore with completed status.
func (s *store) ListCompleted(ctx context.Context, owner string, limit, jobID int) ([]*core.Job, error) {
	var out []*core.Job
	err := s.db.View(func(queryer db.Queryer, binder db.Binder) error {
		limit = limit + 1
		query := queryCompleted
		params := map[string]any{"limit": limit, "owner": owner}
		if jobID != 0 {
			query = queryCompletedOffset
			params["job_id"] = jobID
		}
		stmt, args, err := binder.BindNamed(query, params)
		if err != nil {
			return err
		}
		rows, err := queryer.Query(stmt, args...)
		if err != nil {
			return err
		}
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
	job_url,
	job_author_login,
	job_author_avatar,
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
WHERE job_owner = :job_owner
  AND job_id = :job_id
`

const queryJobIncomplete = queryBase + `
FROM jobs
WHERE job_owner = :owner
  AND job_status IN ('queued', 'in_progress', 'waiting')
ORDER BY job_id DESC
`

const queryCompleted = queryBase + `
FROM jobs
WHERE job_owner = :owner
  AND job_status = 'completed'
ORDER BY job_id DESC
LIMIT :limit
`

const queryCompletedOffset = queryBase + `
FROM jobs
WHERE job_owner = :owner
  AND job_status = 'completed'
  AND job_id > :job_id
ORDER BY job_id DESC
LIMIT :limit
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
	job_url,
	job_author_login,
	job_author_avatar,
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
	:job_url,
	:job_author_login,
	:job_author_avatar,
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
	job_url             = :job_url,
	job_author_login    = :job_author_login,
	job_author_avatar   = :job_author_avatar,
	job_queued          = :job_queued,
	job_started         = :job_started,
	job_completed       = :job_completed,
	job_updated         = :job_updated,
	job_version         = :job_version_new
WHERE job_id = :job_id
AND job_version = :job_version_old
`
