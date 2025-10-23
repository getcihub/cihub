package job

import (
	"context"
	"testing"

	"github.com/getcihub/cihub/core"
	"github.com/getcihub/cihub/store/shared/db"
	"github.com/getcihub/cihub/store/shared/db/dbtest"
)

var noContext = context.TODO()

func TestJob(t *testing.T) {
	conn, err := dbtest.Connect()
	if err != nil {
		t.Error(err)
		return
	}
	defer func() {
		dbtest.Reset(conn)
		dbtest.Disconnect(conn)
	}()

	store := New(conn).(*store)

	t.Run("Count", testJobCount(store))
	t.Run("Create", testJobCreate(store))
	t.Run("Find", testJobFind(store))
	t.Run("FindRunID", testJobFindRunID(store))
	t.Run("List", testJobList(store))
	t.Run("ListStatus", testJobListStatus(store))
	t.Run("Update", testJobUpdate(store))
	t.Run("Delete", testJobDelete(store))
	t.Run("Purge", testJobPurge(store))
}

func testJobCount(store *store) func(t *testing.T) {
	return func(t *testing.T) {
		count, err := store.Count(noContext)
		if err != nil {
			t.Error(err)
		}
		if got, want := count, int64(0); got != want {
			t.Errorf("Want job count %d, got %d", want, got)
		}
	}
}

func testJobCreate(store *store) func(t *testing.T) {
	return func(t *testing.T) {
		job := &core.Job{
			ID:             1001,
			RunID:          2001,
			InstallationID: 3001,
			Owner:          "octocat",
			Repo:           "hello-world",
			Workflow:       "CI",
			Status:         core.JobStatusQueued,
			Labels:         []string{"self-hosted", "linux"},
			URL:            "https://github.com/octocat/example/actions/runs/18634449387/job/53123652430",
			Queued:         1234567890,
			Created:        1234567890,
			Updated:        1234567890,
			Version:        0,
		}
		if err := store.Create(noContext, job); err != nil {
			t.Error(err)
		}
		count, _ := store.Count(noContext)
		if got, want := count, int64(1); got != want {
			t.Errorf("Want job count %d, got %d", want, got)
		}
	}
}

func testJobFind(store *store) func(t *testing.T) {
	return func(t *testing.T) {
		job, err := store.Find(noContext, 1001)
		if err != nil {
			t.Error(err)
			return
		}
		if got, want := job.ID, int64(1001); got != want {
			t.Errorf("Want job ID %d, got %d", want, got)
		}
		if got, want := job.Owner, "octocat"; got != want {
			t.Errorf("Want job owner %s, got %s", want, got)
		}
		if got, want := len(job.Labels), 2; got != want {
			t.Errorf("Want %d labels, got %d", want, got)
		}
	}
}

func testJobFindRunID(store *store) func(t *testing.T) {
	return func(t *testing.T) {
		// Create another job with the same run ID
		job2 := &core.Job{
			ID:             1002,
			RunID:          2001,
			InstallationID: 3001,
			Owner:          "octocat",
			Repo:           "hello-world",
			Workflow:       "CI",
			Status:         core.JobStatusQueued,
			Labels:         []string{"self-hosted"},
			Created:        1234567891,
			Updated:        1234567891,
		}
		if err := store.Create(noContext, job2); err != nil {
			t.Error(err)
			return
		}

		jobs, err := store.FindRunID(noContext, 2001)
		if err != nil {
			t.Error(err)
			return
		}
		if got, want := len(jobs), 2; got != want {
			t.Errorf("Want %d jobs for run ID, got %d", want, got)
		}
	}
}

func testJobList(store *store) func(t *testing.T) {
	return func(t *testing.T) {
		jobs, err := store.List(noContext, core.JobParams{Limit: 10})
		if err != nil {
			t.Error(err)
			return
		}
		if got, want := len(jobs), 2; got != want {
			t.Errorf("Want %d jobs, got %d", want, got)
		}
	}
}

func testJobListStatus(store *store) func(t *testing.T) {
	return func(t *testing.T) {
		jobs, err := store.ListStatus(noContext, core.JobStatusQueued)
		if err != nil {
			t.Error(err)
			return
		}
		if got, want := len(jobs), 2; got != want {
			t.Errorf("Want %d queued jobs, got %d", want, got)
		}
	}
}

func testJobUpdate(store *store) func(t *testing.T) {
	return func(t *testing.T) {
		job, err := store.Find(noContext, 1001)
		if err != nil {
			t.Error(err)
			return
		}

		job.Status = core.JobStatusInProgress
		job.RunnerID = 5001
		job.RunnerName = "runner-123"
		job.Started = 1234567900

		if err := store.Update(noContext, job); err != nil {
			t.Error(err)
			return
		}

		updated, err := store.Find(noContext, 1001)
		if err != nil {
			t.Error(err)
			return
		}

		if got, want := updated.Status, core.JobStatusInProgress; got != want {
			t.Errorf("Want status %s, got %s", want, got)
		}
		if got, want := updated.RunnerID, int64(5001); got != want {
			t.Errorf("Want runner ID %d, got %d", want, got)
		}
		if got, want := updated.Version, int64(1); got != want {
			t.Errorf("Want version %d, got %d", want, got)
		}

		// Test optimistic locking - should fail with old version
		staleJob := *updated
		staleJob.Version = 0
		if err := store.Update(noContext, &staleJob); err != db.ErrOptimisticLock {
			t.Errorf("Expected optimistic lock error, got %v", err)
		}
	}
}

func testJobDelete(store *store) func(t *testing.T) {
	return func(t *testing.T) {
		job, err := store.Find(noContext, 1002)
		if err != nil {
			t.Error(err)
			return
		}

		if err := store.Delete(noContext, job); err != nil {
			t.Error(err)
			return
		}

		_, err = store.Find(noContext, 1002)
		if err == nil {
			t.Errorf("Expected error finding deleted job")
		}

		count, _ := store.Count(noContext)
		if got, want := count, int64(1); got != want {
			t.Errorf("Want job count %d after delete, got %d", want, got)
		}
	}
}

func testJobPurge(store *store) func(t *testing.T) {
	return func(t *testing.T) {
		// Create a completed job
		oldJob := &core.Job{
			ID:             1003,
			RunID:          2002,
			InstallationID: 3001,
			Owner:          "octocat",
			Repo:           "test-repo",
			Workflow:       "Test",
			Status:         core.JobStatusCompleted,
			Conclusion:     "success",
			Labels:         []string{"linux"},
			Completed:      1000000000, // Old timestamp
			Created:        1000000000,
			Updated:        1000000000,
		}
		if err := store.Create(noContext, oldJob); err != nil {
			t.Error(err)
			return
		}

		// Purge jobs completed before timestamp 2000000000
		if err := store.Purge(noContext, 2000000000); err != nil {
			t.Error(err)
			return
		}

		// Old completed job should be deleted
		_, err := store.Find(noContext, 1003)
		if err == nil {
			t.Errorf("Expected error finding purged job")
		}

		// In-progress job should still exist
		_, err = store.Find(noContext, 1001)
		if err != nil {
			t.Errorf("In-progress job should not be purged: %v", err)
		}
	}
}
