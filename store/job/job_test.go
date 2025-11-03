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
	t.Run("Update", testJobUpdate(store))
	t.Run("ListIncomplete", testJobListIncomplete(store))
	t.Run("ListCompleted", testJobListCompleted(store))
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
		job, err := store.Find(noContext, "octocat", 1001)
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

func testJobUpdate(store *store) func(t *testing.T) {
	return func(t *testing.T) {
		job, err := store.Find(noContext, "octocat", 1001)
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

		updated, err := store.Find(noContext, "octocat", 1001)
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
		// Create a job to delete
		jobToDelete := &core.Job{
			ID:             9001,
			RunID:          9001,
			InstallationID: 3001,
			Owner:          "delete-test",
			Repo:           "test-repo",
			Workflow:       "Test",
			Status:         core.JobStatusQueued,
			Labels:         []string{"self-hosted"},
			URL:            "https://github.com/delete-test/test-repo/actions/runs/9001",
			Queued:         1234567890,
			Created:        1234567890,
			Updated:        1234567890,
			Version:        0,
		}

		if err := store.Create(noContext, jobToDelete); err != nil {
			t.Fatalf("Failed to create job for deletion: %v", err)
		}

		// Delete the job
		if err := store.Delete(noContext, jobToDelete); err != nil {
			t.Error(err)
			return
		}

		// Verify it's gone
		_, err := store.Find(noContext, "delete-test", 9001)
		if err == nil {
			t.Errorf("Expected error finding deleted job")
		}
	}
}

func testJobPurge(store *store) func(t *testing.T) {
	return func(t *testing.T) {
		// Create an old completed job
		oldJob := &core.Job{
			ID:             9002,
			RunID:          9002,
			InstallationID: 3001,
			Owner:          "purge-test",
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
			t.Fatalf("Failed to create old job: %v", err)
		}

		// Create a recent completed job that should NOT be purged
		recentJob := &core.Job{
			ID:             9003,
			RunID:          9003,
			InstallationID: 3001,
			Owner:          "purge-test",
			Repo:           "test-repo",
			Workflow:       "Test",
			Status:         core.JobStatusCompleted,
			Conclusion:     "success",
			Labels:         []string{"linux"},
			Completed:      2500000000, // Recent timestamp
			Created:        2500000000,
			Updated:        2500000000,
		}
		if err := store.Create(noContext, recentJob); err != nil {
			t.Fatalf("Failed to create recent job: %v", err)
		}

		// Purge jobs completed before timestamp 2000000000
		if err := store.Purge(noContext, 2000000000); err != nil {
			t.Fatalf("Purge failed: %v", err)
		}

		// Old completed job should be deleted
		_, err := store.Find(noContext, "purge-test", 9002)
		if err == nil {
			t.Errorf("Expected error finding purged job")
		}

		// Recent completed job should still exist
		_, err = store.Find(noContext, "purge-test", 9003)
		if err != nil {
			t.Errorf("Recent job should not be purged: %v", err)
		}
	}
}

func testJobListIncomplete(store *store) func(t *testing.T) {
	return func(t *testing.T) {
		// Create multiple jobs with different statuses for different owners
		incompleteJobs := []*core.Job{
			{
				ID:             2001,
				RunID:          2001,
				InstallationID: 3001,
				Owner:          "octocat",
				Repo:           "hello-world",
				Workflow:       "CI",
				Status:         core.JobStatusQueued,
				Labels:         []string{"self-hosted"},
				URL:            "https://github.com/octocat/example/actions/runs/2001",
				Queued:         1234567890,
				Created:        1234567890,
				Updated:        1234567890,
				Version:        0,
			},
			{
				ID:             2002,
				RunID:          2002,
				InstallationID: 3001,
				Owner:          "octocat",
				Repo:           "hello-world",
				Workflow:       "CI",
				Status:         core.JobStatusInProgress,
				Labels:         []string{"self-hosted"},
				URL:            "https://github.com/octocat/example/actions/runs/2002",
				Queued:         1234567891,
				Started:        1234567892,
				Created:        1234567891,
				Updated:        1234567892,
				Version:        0,
			},
			{
				ID:             2003,
				RunID:          2003,
				InstallationID: 3001,
				Owner:          "octocat",
				Repo:           "hello-world",
				Workflow:       "CI",
				Status:         core.JobStatusWaiting,
				Labels:         []string{"self-hosted"},
				URL:            "https://github.com/octocat/example/actions/runs/2003",
				Queued:         1234567893,
				Created:        1234567893,
				Updated:        1234567893,
				Version:        0,
			},
			{
				ID:             2004,
				RunID:          2004,
				InstallationID: 3001,
				Owner:          "octocat",
				Repo:           "hello-world",
				Workflow:       "CI",
				Status:         core.JobStatusCompleted,
				Conclusion:     "success",
				Labels:         []string{"self-hosted"},
				URL:            "https://github.com/octocat/example/actions/runs/2004",
				Queued:         1234567894,
				Started:        1234567895,
				Completed:      1234567900,
				Created:        1234567894,
				Updated:        1234567900,
				Version:        0,
			},
			{
				ID:             2005,
				RunID:          2005,
				InstallationID: 3001,
				Owner:          "other-user",
				Repo:           "test-repo",
				Workflow:       "Test",
				Status:         core.JobStatusQueued,
				Labels:         []string{"self-hosted"},
				URL:            "https://github.com/other-user/test-repo/actions/runs/2005",
				Queued:         1234567901,
				Created:        1234567901,
				Updated:        1234567901,
				Version:        0,
			},
		}

		for _, job := range incompleteJobs {
			if err := store.Create(noContext, job); err != nil {
				t.Fatalf("Failed to create job %d: %v", job.ID, err)
			}
		}

		// Test: ListIncomplete for octocat should return queued, in_progress, waiting but not completed
		jobs, err := store.ListIncomplete(noContext, "octocat")
		if err != nil {
			t.Fatalf("ListIncomplete failed: %v", err)
		}

		// Job 1001 was marked in_progress in testJobUpdate, plus we created jobs 2001-2003
		if got, want := len(jobs), 4; got != want {
			t.Errorf("Want %d incomplete jobs for octocat, got %d", want, got)
		}

		// Find the jobs we created in this test (IDs 2001-2003)
		var testJobs []*core.Job
		for _, job := range jobs {
			if job.ID >= 2001 && job.ID <= 2003 {
				testJobs = append(testJobs, job)
			}
		}

		if got, want := len(testJobs), 3; got != want {
			t.Errorf("Want %d test incomplete jobs for octocat, got %d", want, got)
		}

		// Verify the test jobs are ordered by ID and have correct statuses
		expectedIDs := []int64{2001, 2002, 2003}
		expectedStatuses := []string{core.JobStatusQueued, core.JobStatusInProgress, core.JobStatusWaiting}

		for i, job := range testJobs {
			if got, want := job.ID, expectedIDs[i]; got != want {
				t.Errorf("Job %d: want ID %d, got %d", i, want, got)
			}
			if got, want := job.Status, expectedStatuses[i]; got != want {
				t.Errorf("Job %d: want status %s, got %s", i, want, got)
			}
			if got, want := job.Owner, "octocat"; got != want {
				t.Errorf("Job %d: want owner %s, got %s", i, want, got)
			}
		}

		// Test: ListIncomplete for other-user should only return their incomplete jobs
		otherJobs, err := store.ListIncomplete(noContext, "other-user")
		if err != nil {
			t.Fatalf("ListIncomplete for other-user failed: %v", err)
		}

		if got, want := len(otherJobs), 1; got != want {
			t.Errorf("Want %d incomplete jobs for other-user, got %d", want, got)
		}

		if got, want := otherJobs[0].ID, int64(2005); got != want {
			t.Errorf("Want job ID %d, got %d", want, got)
		}

		// Test: ListIncomplete for non-existent owner should return empty list
		noJobs, err := store.ListIncomplete(noContext, "nonexistent")
		if err != nil {
			t.Fatalf("ListIncomplete for nonexistent owner failed: %v", err)
		}

		if got, want := len(noJobs), 0; got != want {
			t.Errorf("Want %d jobs for nonexistent owner, got %d", want, got)
		}
	}
}

func testJobListCompleted(store *store) func(t *testing.T) {
	return func(t *testing.T) {
		// Create multiple completed jobs for testing pagination
		completedJobs := []*core.Job{
			{
				ID:             3001,
				RunID:          3001,
				InstallationID: 3001,
				Owner:          "octocat",
				Repo:           "hello-world",
				Workflow:       "CI",
				Status:         core.JobStatusCompleted,
				Conclusion:     "success",
				Labels:         []string{"self-hosted"},
				URL:            "https://github.com/octocat/example/actions/runs/3001",
				Completed:      1234568000,
				Created:        1234567990,
				Updated:        1234568000,
				Version:        0,
			},
			{
				ID:             3002,
				RunID:          3002,
				InstallationID: 3001,
				Owner:          "octocat",
				Repo:           "hello-world",
				Workflow:       "CI",
				Status:         core.JobStatusCompleted,
				Conclusion:     "success",
				Labels:         []string{"self-hosted"},
				URL:            "https://github.com/octocat/example/actions/runs/3002",
				Completed:      1234568001,
				Created:        1234567991,
				Updated:        1234568001,
				Version:        0,
			},
			{
				ID:             3003,
				RunID:          3003,
				InstallationID: 3001,
				Owner:          "octocat",
				Repo:           "hello-world",
				Workflow:       "CI",
				Status:         core.JobStatusCompleted,
				Conclusion:     "success",
				Labels:         []string{"self-hosted"},
				URL:            "https://github.com/octocat/example/actions/runs/3003",
				Completed:      1234568002,
				Created:        1234567992,
				Updated:        1234568002,
				Version:        0,
			},
			{
				ID:             3004,
				RunID:          3004,
				InstallationID: 3001,
				Owner:          "octocat",
				Repo:           "hello-world",
				Workflow:       "CI",
				Status:         core.JobStatusCompleted,
				Conclusion:     "success",
				Labels:         []string{"self-hosted"},
				URL:            "https://github.com/octocat/example/actions/runs/3004",
				Completed:      1234568003,
				Created:        1234567993,
				Updated:        1234568003,
				Version:        0,
			},
			{
				ID:             3005,
				RunID:          3005,
				InstallationID: 3001,
				Owner:          "other-user",
				Repo:           "test-repo",
				Workflow:       "Test",
				Status:         core.JobStatusCompleted,
				Conclusion:     "success",
				Labels:         []string{"self-hosted"},
				URL:            "https://github.com/other-user/test-repo/actions/runs/3005",
				Completed:      1234568004,
				Created:        1234567994,
				Updated:        1234568004,
				Version:        0,
			},
		}

		for _, job := range completedJobs {
			if err := store.Create(noContext, job); err != nil {
				t.Fatalf("Failed to create job %d: %v", job.ID, err)
			}
		}

		// Test: ListCompleted with limit for octocat
		jobs, err := store.ListCompleted(noContext, "octocat", 2, 0)
		if err != nil {
			t.Fatalf("ListCompleted failed: %v", err)
		}

		// Job 2004 was created in ListIncomplete test, plus jobs 3001-3004
		// Filter to only the jobs created in this test
		var testJobs []*core.Job
		for _, job := range jobs {
			if job.ID >= 3001 && job.ID <= 3004 {
				testJobs = append(testJobs, job)
			}
		}

		// Should return limit (2) jobs since there are more than limit
		if got, want := len(testJobs), 2; got != want {
			t.Errorf("Want %d completed jobs with limit 2, got %d", want, got)
		}

		if got, want := testJobs[0].ID, int64(3001); got != want {
			t.Errorf("Want first job ID %d, got %d", want, got)
		}
		if got, want := testJobs[1].ID, int64(3002); got != want {
			t.Errorf("Want second job ID %d, got %d", want, got)
		}

		// Test: ListCompleted with offset (cursor-based pagination)
		// Note: the implementation adds 1 to limit internally, so limit=2 becomes limit=3
		nextJobs, err := store.ListCompleted(noContext, "octocat", 2, 3001)
		if err != nil {
			t.Fatalf("ListCompleted with offset failed: %v", err)
		}

		// Filter to only test jobs
		var testNextJobs []*core.Job
		for _, job := range nextJobs {
			if job.ID >= 3001 && job.ID <= 3004 {
				testNextJobs = append(testNextJobs, job)
			}
		}

		// Should return jobs after 3001. With limit=2, we get limit+1=3, so 3002, 3003, 3004
		if got, want := len(testNextJobs), 3; got != want {
			t.Errorf("Want %d completed jobs after cursor 3001, got %d", want, got)
		}

		if got, want := testNextJobs[0].ID, int64(3002); got != want {
			t.Errorf("Want first job ID %d, got %d", want, got)
		}
		if got, want := testNextJobs[1].ID, int64(3003); got != want {
			t.Errorf("Want second job ID %d, got %d", want, got)
		}
		if got, want := testNextJobs[2].ID, int64(3004); got != want {
			t.Errorf("Want third job ID %d, got %d", want, got)
		}

		// Test: ListCompleted with offset at end of list
		lastJobs, err := store.ListCompleted(noContext, "octocat", 2, 3003)
		if err != nil {
			t.Fatalf("ListCompleted with offset at end failed: %v", err)
		}

		// Filter to only test jobs
		var testLastJobs []*core.Job
		for _, job := range lastJobs {
			if job.ID >= 3001 && job.ID <= 3004 {
				testLastJobs = append(testLastJobs, job)
			}
		}

		// Should return only the remaining job (3004)
		if got, want := len(testLastJobs), 1; got != want {
			t.Errorf("Want %d completed jobs after cursor 3003, got %d", want, got)
		}

		if got, want := testLastJobs[0].ID, int64(3004); got != want {
			t.Errorf("Want job ID %d, got %d", want, got)
		}

		// Test: ListCompleted for other-user returns their jobs only
		otherJobs, err := store.ListCompleted(noContext, "other-user", 10, 0)
		if err != nil {
			t.Fatalf("ListCompleted for other-user failed: %v", err)
		}

		if got, want := len(otherJobs), 1; got != want {
			t.Errorf("Want %d completed jobs for other-user, got %d", want, got)
		}

		if got, want := otherJobs[0].ID, int64(3005); got != want {
			t.Errorf("Want job ID %d, got %d", want, got)
		}

		// Test: ListCompleted for non-existent owner returns empty list
		noJobs, err := store.ListCompleted(noContext, "nonexistent", 10, 0)
		if err != nil {
			t.Fatalf("ListCompleted for nonexistent owner failed: %v", err)
		}

		if got, want := len(noJobs), 0; got != want {
			t.Errorf("Want %d jobs for nonexistent owner, got %d", want, got)
		}
	}
}
