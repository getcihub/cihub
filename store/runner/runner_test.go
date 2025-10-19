package runner

import (
	"context"
	"testing"
	"time"

	"github.com/getcihub/cihub/core"
	"github.com/getcihub/cihub/store/shared/db/dbtest"
	"github.com/getcihub/cihub/store/shared/encrypter"
)

var noContext = context.TODO()

func TestRunner(t *testing.T) {
	conn, err := dbtest.Connect()
	if err != nil {
		t.Error(err)
		return
	}
	defer func() {
		dbtest.Reset(conn)
		dbtest.Disconnect(conn)
	}()

	store := New(conn, nil).(*store)
	store.enc, _ = encrypter.New("fb4b4d6267c8a5ce8231f8b186dbca92")
	t.Run("Create", testRunnerCreate(store))
}

func testRunnerCreate(store *store) func(t *testing.T) {
	return func(t *testing.T) {
		now := time.Now().Unix()
		runner := &core.Runner{
			Name:           "runner-test-001",
			ID:             12345,
			InstallationID: 67890,
			Status:         core.RunnerStatusPending,
			AssignedTo:     111,
			Cancelled:      false,
			Created:        now,
			Updated:        now,
			Timeout:        300,
			Token:          "s.abcdef123456",
		}
		err := store.Create(noContext, runner)
		if err != nil {
			t.Error(err)
		}

		t.Run("Count", testRunnerCount(store))
		t.Run("Find", testRunnerFind(store, runner))
		t.Run("FindID", testRunnerFindID(store, runner))
		t.Run("FindAssignedTo", testRunnerFindAssignedTo(store, runner))
		t.Run("List", testRunnerList(store))
		t.Run("ListStatus", testRunnerListStatus(store))
		t.Run("Update", testRunnerUpdate(store, runner))
		t.Run("Purge", testRunnerPurge(store, runner))
		t.Run("Delete", testRunnerDelete(store, runner))
	}
}

func testRunnerCount(runners *store) func(t *testing.T) {
	return func(t *testing.T) {
		count, err := runners.Count(noContext)
		if err != nil {
			t.Error(err)
		}
		if got, want := count, int64(1); got != want {
			t.Errorf("Want runner table count %d, got %d", want, got)
		}
	}
}

func testRunnerFind(runners *store, created *core.Runner) func(t *testing.T) {
	return func(t *testing.T) {
		runner, err := runners.Find(noContext, created.Name)
		if err != nil {
			t.Error(err)
		} else {
			t.Run("Fields", testRunner(runner))
		}
	}
}

func testRunnerFindID(runners *store, created *core.Runner) func(t *testing.T) {
	return func(t *testing.T) {
		runner, err := runners.FindID(noContext, created.ID)
		if err != nil {
			t.Error(err)
		}
		if runner.Name != created.Name {
			t.Errorf("Want runner name %s, got %s", created.Name, runner.Name)
		}
	}
}

func testRunnerFindAssignedTo(runners *store, created *core.Runner) func(t *testing.T) {
	return func(t *testing.T) {
		runner, err := runners.FindAssignedTo(noContext, created.AssignedTo)
		if err != nil {
			t.Error(err)
		}
		if runner.Name != created.Name {
			t.Errorf("Want runner name %s, got %s", created.Name, runner.Name)
		}
	}
}

func testRunnerList(runners *store) func(t *testing.T) {
	return func(t *testing.T) {
		list, err := runners.List(noContext, core.RunnerParams{Limit: 100})
		if err != nil {
			t.Error(err)
			return
		}
		if got, want := len(list), 1; got != want {
			t.Errorf("Want runner count %d, got %d", want, got)
		}
	}
}

func testRunnerListStatus(runners *store) func(t *testing.T) {
	return func(t *testing.T) {
		list, err := runners.ListStatus(noContext, core.RunnerStatusPending, core.RunnerParams{Limit: 100})
		if err != nil {
			t.Error(err)
			return
		}
		if got, want := len(list), 1; got != want {
			t.Errorf("Want runner count %d, got %d", want, got)
		}
		if list[0].Status != core.RunnerStatusPending {
			t.Errorf("Want status %s, got %s", core.RunnerStatusPending, list[0].Status)
		}
	}
}

func testRunnerUpdate(runners *store, created *core.Runner) func(t *testing.T) {
	return func(t *testing.T) {
		// Fetch the current state
		runner, err := runners.Find(noContext, created.Name)
		if err != nil {
			t.Error(err)
			return
		}

		// Update fields
		runner.Status = core.RunnerStatusBusy
		runner.AssignedTo = 333
		runner.Started = time.Now().Unix()
		runner.Updated = time.Now().Unix()

		err = runners.Update(noContext, runner)
		if err != nil {
			t.Error(err)
			return
		}

		// Verify the update
		updated, err := runners.Find(noContext, runner.Name)
		if err != nil {
			t.Error(err)
			return
		}
		if updated.Status != core.RunnerStatusBusy {
			t.Errorf("Want status %s, got %s", core.RunnerStatusBusy, updated.Status)
		}
		if updated.AssignedTo != 333 {
			t.Errorf("Want assigned_to %d, got %d", 333, updated.AssignedTo)
		}

		// Copy updated runner to created for other tests
		*created = *updated
	}
}

func testRunnerPurge(runners *store, created *core.Runner) func(t *testing.T) {
	return func(t *testing.T) {
		// Create a completed runner with old stopped timestamp
		oldRunner := &core.Runner{
			Name:           "runner-old-001",
			ID:             99999,
			InstallationID: created.InstallationID,
			Status:         core.RunnerStatusCompleted,
			AssignedTo:     444,
			Created:        time.Now().Add(-2 * time.Hour).Unix(),
			Stopped:        time.Now().Add(-1 * time.Hour).Unix(),
			Token:          "s.old123",
		}
		err := runners.Create(noContext, oldRunner)
		if err != nil {
			t.Error(err)
			return
		}

		// Purge runners stopped before 30 minutes ago
		purgeTime := time.Now().Add(-30 * time.Minute).Unix()
		err = runners.Purge(noContext, purgeTime)
		if err != nil {
			t.Error(err)
			return
		}

		// Old runner should be deleted
		_, err = runners.Find(noContext, oldRunner.Name)
		if err == nil {
			t.Error("Expected old runner to be purged")
		}

		// Original runner should still exist (not completed/stopped)
		_, err = runners.Find(noContext, created.Name)
		if err != nil {
			t.Error("Expected original runner to still exist")
		}
	}
}

func testRunnerDelete(runners *store, created *core.Runner) func(t *testing.T) {
	return func(t *testing.T) {
		err := runners.Delete(noContext, created)
		if err != nil {
			t.Error(err)
		}
	}
}

func testRunner(runner *core.Runner) func(t *testing.T) {
	return func(t *testing.T) {
		if got, want := runner.Name, "runner-test-001"; got != want {
			t.Errorf("Want runner name %s, got %s", want, got)
		}
		if got, want := runner.ID, int64(12345); got != want {
			t.Errorf("Want runner ID %d, got %d", want, got)
		}
		if got, want := runner.Status, core.RunnerStatusPending; got != want {
			t.Errorf("Want status %s, got %s", want, got)
		}
		if got, want := runner.Token, "s.abcdef123456"; got != want {
			t.Errorf("Want token %s, got %s", want, got)
		}
	}
}
