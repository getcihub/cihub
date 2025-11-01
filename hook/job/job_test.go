package job

import (
	"context"
	"encoding/json"
	"errors"
	"testing"
	"time"

	"github.com/google/go-github/v76/github"
	"go.uber.org/mock/gomock"

	"github.com/getcihub/cihub/core"
	"github.com/getcihub/cihub/mock"
)

var noContext = context.TODO()

// testLabels is not used anymore since labels are parsed dynamically.
// Keeping this function for backwards compatibility with test structure.

func TestHandler_Handles(t *testing.T) {
	ctrl := gomock.NewController(t)
	defer ctrl.Finish()

	mockJobStore := mock.NewMockJobStore(ctrl)
	mockRunnerStore := mock.NewMockRunnerStore(ctrl)
	mockScheduler := mock.NewMockScheduler(ctrl)
	h := New(mockJobStore, mockRunnerStore, mockScheduler)

	events := h.Handles()
	if got, want := len(events), 1; got != want {
		t.Errorf("Want %d handled events, got %d", want, got)
	}
	if got, want := events[0], "workflow_job"; got != want {
		t.Errorf("Want event type %s, got %s", want, got)
	}
}

func TestHandler_Handle_Waiting_CreateNew(t *testing.T) {
	ctrl := gomock.NewController(t)
	defer ctrl.Finish()

	mockJobStore := mock.NewMockJobStore(ctrl)
	mockRunnerStore := mock.NewMockRunnerStore(ctrl)
	mockScheduler := mock.NewMockScheduler(ctrl)
	h := New(mockJobStore, mockRunnerStore, mockScheduler)

	now := time.Now()
	event := &github.WorkflowJobEvent{
		Action: github.Ptr("waiting"),
		WorkflowJob: &github.WorkflowJob{
			ID:           github.Ptr(int64(1001)),
			RunID:        github.Ptr(int64(2001)),
			WorkflowName: github.Ptr("CI"),
			Status:       github.Ptr(core.JobStatusQueued),
			HeadBranch:   github.Ptr("main"),
			HeadSHA:      github.Ptr("abc123"),
			Labels:       []string{"cihub-2cpu-2048mb"},
			HTMLURL:      github.Ptr("https://github.com/octocat/hello-world/actions/runs/2001/jobs/1001"),
			CreatedAt:    &github.Timestamp{Time: now},
		},
		Repo: &github.Repository{
			Name: github.Ptr("hello-world"),
			Owner: &github.User{
				Login: github.Ptr("octocat"),
			},
		},
		Installation: &github.Installation{
			ID: github.Ptr(int64(3001)),
		},
	}

	payload, err := json.Marshal(event)
	if err != nil {
		t.Fatal(err)
	}

	// Expect Find to return error (job doesn't exist)
	mockJobStore.EXPECT().
		Find(gomock.Any(), int64(1001)).
		Return(nil, errors.New("not found"))

	// Expect Create to be called
	mockJobStore.EXPECT().
		Create(gomock.Any(), gomock.Any()).
		DoAndReturn(func(ctx context.Context, job *core.Job) error {
			// Verify conversion
			if got, want := job.ID, int64(1001); got != want {
				t.Errorf("Want job ID %d, got %d", want, got)
			}
			if got, want := job.Status, core.JobStatusQueued; got != want {
				t.Errorf("Want status %s, got %s", want, got)
			}
			if job.Created == 0 {
				t.Error("Expected Created timestamp to be set")
			}
			if job.Updated == 0 {
				t.Error("Expected Updated timestamp to be set")
			}
			return nil
		})

	// Schedule should NOT be called for waiting action
	// (no expectation set, will fail if called)

	if err := h.Handle(noContext, "workflow_job", "test-delivery", payload); err != nil {
		t.Errorf("Handle failed: %v", err)
	}
}

func TestHandler_Handle_Waiting_AlreadyExists(t *testing.T) {
	ctrl := gomock.NewController(t)
	defer ctrl.Finish()

	mockJobStore := mock.NewMockJobStore(ctrl)
	mockRunnerStore := mock.NewMockRunnerStore(ctrl)
	mockScheduler := mock.NewMockScheduler(ctrl)
	h := New(mockJobStore, mockRunnerStore, mockScheduler)

	event := &github.WorkflowJobEvent{
		Action: github.Ptr("waiting"),
		WorkflowJob: &github.WorkflowJob{
			ID:           github.Ptr(int64(1001)),
			RunID:        github.Ptr(int64(2001)),
			WorkflowName: github.Ptr("CI"),
			Status:       github.Ptr(core.JobStatusQueued),
			Labels:       []string{"cihub-2cpu-2048mb"},
		},
		Repo: &github.Repository{
			Name: github.Ptr("hello-world"),
			Owner: &github.User{
				Login: github.Ptr("octocat"),
			},
		},
		Installation: &github.Installation{
			ID: github.Ptr(int64(3001)),
		},
	}

	payload, err := json.Marshal(event)
	if err != nil {
		t.Fatal(err)
	}

	existingJob := &core.Job{
		ID:      1001,
		Created: time.Now().Unix(),
	}

	// Expect Find to return existing job
	mockJobStore.EXPECT().
		Find(gomock.Any(), int64(1001)).
		Return(existingJob, nil)

	// Create should NOT be called (no expectation set)
	// Schedule should NOT be called (no expectation set)

	if err := h.Handle(noContext, "workflow_job", "test-delivery", payload); err != nil {
		t.Errorf("Handle failed: %v", err)
	}
}

func TestHandler_Handle_Queued_CreateNew(t *testing.T) {
	ctrl := gomock.NewController(t)
	defer ctrl.Finish()

	mockJobStore := mock.NewMockJobStore(ctrl)
	mockRunnerStore := mock.NewMockRunnerStore(ctrl)
	mockScheduler := mock.NewMockScheduler(ctrl)
	h := New(mockJobStore, mockRunnerStore, mockScheduler)

	now := time.Now()
	event := &github.WorkflowJobEvent{
		Action: github.Ptr("queued"),
		WorkflowJob: &github.WorkflowJob{
			ID:           github.Ptr(int64(1001)),
			RunID:        github.Ptr(int64(2001)),
			WorkflowName: github.Ptr("CI"),
			Status:       github.Ptr(core.JobStatusQueued),
			HeadBranch:   github.Ptr("main"),
			HeadSHA:      github.Ptr("abc123"),
			Labels:       []string{"cihub-2cpu-2048mb"},
			HTMLURL:      github.Ptr("https://github.com/octocat/hello-world/actions/runs/2001/jobs/1001"),
			CreatedAt:    &github.Timestamp{Time: now},
		},
		Repo: &github.Repository{
			Name: github.Ptr("hello-world"),
			Owner: &github.User{
				Login: github.Ptr("octocat"),
			},
		},
		Installation: &github.Installation{
			ID: github.Ptr(int64(3001)),
		},
	}

	payload, err := json.Marshal(event)
	if err != nil {
		t.Fatal(err)
	}

	// Expect Find to return error (job doesn't exist)
	mockJobStore.EXPECT().
		Find(gomock.Any(), int64(1001)).
		Return(nil, errors.New("not found"))

	// Expect Create to be called
	mockJobStore.EXPECT().
		Create(gomock.Any(), gomock.Any()).
		DoAndReturn(func(ctx context.Context, job *core.Job) error {
			if got, want := job.ID, int64(1001); got != want {
				t.Errorf("Want job ID %d, got %d", want, got)
			}
			return nil
		})

	// Expect runner to be created
	mockRunnerStore.EXPECT().
		Create(gomock.Any(), gomock.Any()).
		Return(nil)

	// Expect Schedule to be called for queued action
	mockScheduler.EXPECT().
		Schedule(gomock.Any(), gomock.Any()).
		Return(nil)

	if err := h.Handle(noContext, "workflow_job", "test-delivery", payload); err != nil {
		t.Errorf("Handle failed: %v", err)
	}
}

func TestHandler_Handle_Queued_UpdateExisting(t *testing.T) {
	ctrl := gomock.NewController(t)
	defer ctrl.Finish()

	mockJobStore := mock.NewMockJobStore(ctrl)
	mockRunnerStore := mock.NewMockRunnerStore(ctrl)
	mockScheduler := mock.NewMockScheduler(ctrl)
	h := New(mockJobStore, mockRunnerStore, mockScheduler)

	createdTime := time.Now().Add(-5 * time.Minute)
	now := time.Now()

	event := &github.WorkflowJobEvent{
		Action: github.Ptr("queued"),
		WorkflowJob: &github.WorkflowJob{
			ID:           github.Ptr(int64(1001)),
			RunID:        github.Ptr(int64(2001)),
			WorkflowName: github.Ptr("CI"),
			Status:       github.Ptr(core.JobStatusQueued),
			HeadBranch:   github.Ptr("main"),
			HeadSHA:      github.Ptr("abc123"),
			Labels:       []string{"cihub-2cpu-2048mb"},
			CreatedAt:    &github.Timestamp{Time: now},
		},
		Repo: &github.Repository{
			Name: github.Ptr("hello-world"),
			Owner: &github.User{
				Login: github.Ptr("octocat"),
			},
		},
		Installation: &github.Installation{
			ID: github.Ptr(int64(3001)),
		},
	}

	payload, err := json.Marshal(event)
	if err != nil {
		t.Fatal(err)
	}

	existingJob := &core.Job{
		ID:      1001,
		RunID:   2001,
		Status:  core.JobStatusQueued,
		Version: 2,
		Created: createdTime.Unix(),
		Updated: createdTime.Unix(),
	}

	// Expect Find to return existing job
	mockJobStore.EXPECT().
		Find(gomock.Any(), int64(1001)).
		Return(existingJob, nil)

	// Expect Update to be called, no runner sync for queued
	mockJobStore.EXPECT().
		Update(gomock.Any(), gomock.Any()).
		DoAndReturn(func(ctx context.Context, job *core.Job) error {
			if got, want := job.Version, int64(2); got != want {
				t.Errorf("Want version %d preserved, got %d", want, got)
			}
			return nil
		})

	// Expect runner to be created
	mockRunnerStore.EXPECT().
		Create(gomock.Any(), gomock.Any()).
		Return(nil)

	// Expect Schedule to be called for queued action
	mockScheduler.EXPECT().
		Schedule(gomock.Any(), gomock.Any()).
		Return(nil)

	if err := h.Handle(noContext, "workflow_job", "test-delivery", payload); err != nil {
		t.Errorf("Handle failed: %v", err)
	}
}

func TestHandler_Handle_InProgress_UpdateExisting(t *testing.T) {
	ctrl := gomock.NewController(t)
	defer ctrl.Finish()

	mockJobStore := mock.NewMockJobStore(ctrl)
	mockRunnerStore := mock.NewMockRunnerStore(ctrl)
	mockScheduler := mock.NewMockScheduler(ctrl)
	h := New(mockJobStore, mockRunnerStore, mockScheduler)

	createdTime := time.Now().Add(-5 * time.Minute)
	startedTime := time.Now()

	event := &github.WorkflowJobEvent{
		Action: github.Ptr("in_progress"),
		WorkflowJob: &github.WorkflowJob{
			ID:           github.Ptr(int64(1001)),
			RunID:        github.Ptr(int64(2001)),
			WorkflowName: github.Ptr("CI"),
			Status:       github.Ptr(core.JobStatusInProgress),
			HeadBranch:   github.Ptr("main"),
			HeadSHA:      github.Ptr("abc123"),
			Labels:       []string{"cihub-2cpu-2048mb"},
			RunnerID:     github.Ptr(int64(5001)),
			RunnerName:   github.Ptr("runner-1"),
			HTMLURL:      github.Ptr("https://github.com/octocat/hello-world/actions/runs/2001/jobs/1001"),
			CreatedAt:    &github.Timestamp{Time: createdTime},
			StartedAt:    &github.Timestamp{Time: startedTime},
		},
		Repo: &github.Repository{
			Name: github.Ptr("hello-world"),
			Owner: &github.User{
				Login: github.Ptr("octocat"),
			},
		},
		Installation: &github.Installation{
			ID: github.Ptr(int64(3001)),
		},
	}

	payload, err := json.Marshal(event)
	if err != nil {
		t.Fatal(err)
	}

	existingJob := &core.Job{
		ID:             1001,
		RunID:          2001,
		InstallationID: 3001,
		Owner:          "octocat",
		Repo:           "hello-world",
		Workflow:       "CI",
		Branch:         "main",
		SHA:            "abc123",
		Status:         core.JobStatusQueued,
		Labels:         []string{"self-hosted", "linux"},
		Queued:         createdTime.Unix(),
		Created:        createdTime.Unix(),
		Updated:        createdTime.Unix(),
		Version:        2,
	}

	// Expect Find to return existing job
	mockJobStore.EXPECT().
		Find(gomock.Any(), int64(1001)).
		Return(existingJob, nil)

	// Expect Update to be called
	mockJobStore.EXPECT().
		Update(gomock.Any(), gomock.Any()).
		DoAndReturn(func(ctx context.Context, job *core.Job) error {
			// Verify update preserves important fields
			if got, want := job.Status, core.JobStatusInProgress; got != want {
				t.Errorf("Want status %s, got %s", want, got)
			}
			if got, want := job.RunnerID, int64(5001); got != want {
				t.Errorf("Want runner ID %d, got %d", want, got)
			}
			if got, want := job.RunnerName, "runner-1"; got != want {
				t.Errorf("Want runner name %s, got %s", want, got)
			}
			if got, want := job.Started, startedTime.Unix(); got != want {
				t.Errorf("Want started timestamp %d, got %d", want, got)
			}
			// Verify preserved fields
			if got, want := job.Version, int64(2); got != want {
				t.Errorf("Want version %d preserved, got %d", want, got)
			}
			if got, want := job.Created, createdTime.Unix(); got != want {
				t.Errorf("Want created timestamp %d preserved, got %d", want, got)
			}
			if job.Updated == createdTime.Unix() {
				t.Error("Expected Updated timestamp to be refreshed")
			}
			return nil
		})

	// Expect runner lookup since RunnerName is set
	mockRunnerStore.EXPECT().
		Find(gomock.Any(), "runner-1").
		Return(&core.Runner{
			Name:   "runner-1",
			ID:     5001,
			Status: core.RunnerStatusIdle,
		}, nil)

	// Expect runner update to mark it as busy
	mockRunnerStore.EXPECT().
		Update(gomock.Any(), gomock.Any()).
		DoAndReturn(func(ctx context.Context, runner *core.Runner) error {
			if runner.Status != core.RunnerStatusBusy {
				t.Errorf("Expected runner status to be busy, got %s", runner.Status)
			}
			if runner.Started == 0 {
				t.Error("Expected runner Started timestamp to be set")
			}
			return nil
		})

	// Schedule should NOT be called for in_progress action

	if err := h.Handle(noContext, "workflow_job", "test-delivery", payload); err != nil {
		t.Errorf("Handle failed: %v", err)
	}
}

func TestHandler_Handle_Completed_UpdateExisting(t *testing.T) {
	ctrl := gomock.NewController(t)
	defer ctrl.Finish()

	mockJobStore := mock.NewMockJobStore(ctrl)
	mockRunnerStore := mock.NewMockRunnerStore(ctrl)
	mockScheduler := mock.NewMockScheduler(ctrl)
	h := New(mockJobStore, mockRunnerStore, mockScheduler)

	createdTime := time.Now().Add(-10 * time.Minute)
	startedTime := time.Now().Add(-5 * time.Minute)
	completedTime := time.Now()

	event := &github.WorkflowJobEvent{
		Action: github.Ptr("completed"),
		WorkflowJob: &github.WorkflowJob{
			ID:           github.Ptr(int64(1001)),
			RunID:        github.Ptr(int64(2001)),
			WorkflowName: github.Ptr("CI"),
			Status:       github.Ptr(core.JobStatusCompleted),
			Conclusion:   github.Ptr("success"),
			HeadBranch:   github.Ptr("main"),
			HeadSHA:      github.Ptr("abc123"),
			Labels:       []string{"cihub-2cpu-2048mb"},
			RunnerID:     github.Ptr(int64(5001)),
			RunnerName:   github.Ptr("runner-1"),
			HTMLURL:      github.Ptr("https://github.com/octocat/hello-world/actions/runs/2001/jobs/1001"),
			CreatedAt:    &github.Timestamp{Time: createdTime},
			StartedAt:    &github.Timestamp{Time: startedTime},
			CompletedAt:  &github.Timestamp{Time: completedTime},
		},
		Repo: &github.Repository{
			Name: github.Ptr("hello-world"),
			Owner: &github.User{
				Login: github.Ptr("octocat"),
			},
		},
		Installation: &github.Installation{
			ID: github.Ptr(int64(3001)),
		},
	}

	payload, err := json.Marshal(event)
	if err != nil {
		t.Fatal(err)
	}

	existingJob := &core.Job{
		ID:             1001,
		RunID:          2001,
		InstallationID: 3001,
		Owner:          "octocat",
		Repo:           "hello-world",
		Status:         core.JobStatusInProgress,
		Version:        3,
		Created:        createdTime.Unix(),
	}

	mockJobStore.EXPECT().
		Find(gomock.Any(), int64(1001)).
		Return(existingJob, nil)

	mockJobStore.EXPECT().
		Update(gomock.Any(), gomock.Any()).
		DoAndReturn(func(ctx context.Context, job *core.Job) error {
			if got, want := job.Status, core.JobStatusCompleted; got != want {
				t.Errorf("Want status %s, got %s", want, got)
			}
			if got, want := job.Conclusion, "success"; got != want {
				t.Errorf("Want conclusion %s, got %s", want, got)
			}
			if got, want := job.Completed, completedTime.Unix(); got != want {
				t.Errorf("Want completed timestamp %d, got %d", want, got)
			}
			return nil
		})

	// Expect Find to find runner by name
	mockRunnerStore.EXPECT().
		Find(gomock.Any(), "runner-1").
		Return(&core.Runner{
			Name:   "runner-1",
			ID:     5001,
			Status: core.RunnerStatusBusy,
		}, nil)

	// Expect Update to mark runner as completed
	mockRunnerStore.EXPECT().
		Update(gomock.Any(), gomock.Any()).
		DoAndReturn(func(ctx context.Context, runner *core.Runner) error {
			if runner.Status != core.RunnerStatusCompleted {
				t.Errorf("Expected runner status to be completed, got %s", runner.Status)
			}
			if runner.Stopped == 0 {
				t.Error("Expected runner Stopped timestamp to be set")
			}
			return nil
		})

	if err := h.Handle(noContext, "workflow_job", "test-delivery", payload); err != nil {
		t.Errorf("Handle failed: %v", err)
	}
}

func TestHandler_Handle_Completed_RunnerNotFound(t *testing.T) {
	ctrl := gomock.NewController(t)
	defer ctrl.Finish()

	mockJobStore := mock.NewMockJobStore(ctrl)
	mockRunnerStore := mock.NewMockRunnerStore(ctrl)
	mockScheduler := mock.NewMockScheduler(ctrl)
	h := New(mockJobStore, mockRunnerStore, mockScheduler)

	createdTime := time.Now().Add(-10 * time.Minute)
	completedTime := time.Now()

	event := &github.WorkflowJobEvent{
		Action: github.Ptr("completed"),
		WorkflowJob: &github.WorkflowJob{
			ID:           github.Ptr(int64(1001)),
			RunID:        github.Ptr(int64(2001)),
			WorkflowName: github.Ptr("CI"),
			Status:       github.Ptr(core.JobStatusCompleted),
			Labels:       []string{"cihub-2cpu-2048mb"},
			RunnerName:   github.Ptr("runner-1"),
			CreatedAt:    &github.Timestamp{Time: createdTime},
			CompletedAt:  &github.Timestamp{Time: completedTime},
		},
		Repo: &github.Repository{
			Name: github.Ptr("hello-world"),
			Owner: &github.User{
				Login: github.Ptr("octocat"),
			},
		},
		Installation: &github.Installation{
			ID: github.Ptr(int64(3001)),
		},
	}

	payload, err := json.Marshal(event)
	if err != nil {
		t.Fatal(err)
	}

	existingJob := &core.Job{
		ID:      1001,
		Status:  core.JobStatusInProgress,
		Version: 1,
		Created: createdTime.Unix(),
	}

	mockJobStore.EXPECT().
		Find(gomock.Any(), int64(1001)).
		Return(existingJob, nil)

	mockJobStore.EXPECT().
		Update(gomock.Any(), gomock.Any()).
		Return(nil)

	// Expect Find to fail when looking up runner
	mockRunnerStore.EXPECT().
		Find(gomock.Any(), "runner-1").
		Return(nil, errors.New("not found"))

	// Handler should not fail if runner is not found (non-fatal error)
	if err := h.Handle(noContext, "workflow_job", "test-delivery", payload); err != nil {
		t.Errorf("Handle failed: %v", err)
	}
}

func TestHandler_Handle_InvalidPayload(t *testing.T) {
	ctrl := gomock.NewController(t)
	defer ctrl.Finish()

	mockJobStore := mock.NewMockJobStore(ctrl)
	mockRunnerStore := mock.NewMockRunnerStore(ctrl)
	mockScheduler := mock.NewMockScheduler(ctrl)
	h := New(mockJobStore, mockRunnerStore, mockScheduler)

	invalidPayload := []byte(`{invalid json}`)

	err := h.Handle(noContext, "workflow_job", "test-delivery", invalidPayload)
	if err == nil {
		t.Error("Expected error for invalid JSON payload")
	}
}

func TestHandler_Handle_MissingWorkflowJob(t *testing.T) {
	ctrl := gomock.NewController(t)
	defer ctrl.Finish()

	mockJobStore := mock.NewMockJobStore(ctrl)
	mockRunnerStore := mock.NewMockRunnerStore(ctrl)
	mockScheduler := mock.NewMockScheduler(ctrl)
	h := New(mockJobStore, mockRunnerStore, mockScheduler)

	event := &github.WorkflowJobEvent{
		Action:      github.Ptr("queued"),
		WorkflowJob: nil, // Missing
		Repo: &github.Repository{
			Name: github.Ptr("hello-world"),
			Owner: &github.User{
				Login: github.Ptr("octocat"),
			},
		},
		Installation: &github.Installation{
			ID: github.Ptr(int64(3001)),
		},
	}

	payload, err := json.Marshal(event)
	if err != nil {
		t.Fatal(err)
	}

	err = h.Handle(noContext, "workflow_job", "test-delivery", payload)
	if err == nil {
		t.Error("Expected error for missing workflow_job")
	}
	if got := err.Error(); got != "missing workflow_job in event payload" {
		t.Errorf("Want error message 'missing workflow_job in event payload', got %q", got)
	}
}

func TestHandler_Handle_MissingRepository(t *testing.T) {
	ctrl := gomock.NewController(t)
	defer ctrl.Finish()

	mockJobStore := mock.NewMockJobStore(ctrl)
	mockRunnerStore := mock.NewMockRunnerStore(ctrl)
	mockScheduler := mock.NewMockScheduler(ctrl)
	h := New(mockJobStore, mockRunnerStore, mockScheduler)

	event := &github.WorkflowJobEvent{
		Action: github.Ptr("queued"),
		WorkflowJob: &github.WorkflowJob{
			ID: github.Ptr(int64(1001)),
		},
		Repo: nil, // Missing
		Installation: &github.Installation{
			ID: github.Ptr(int64(3001)),
		},
	}

	payload, err := json.Marshal(event)
	if err != nil {
		t.Fatal(err)
	}

	err = h.Handle(noContext, "workflow_job", "test-delivery", payload)
	if err == nil {
		t.Error("Expected error for missing repository")
	}
	if got := err.Error(); got != "missing repository in event payload" {
		t.Errorf("Want error message 'missing repository in event payload', got %q", got)
	}
}

func TestHandler_Handle_MissingInstallation(t *testing.T) {
	ctrl := gomock.NewController(t)
	defer ctrl.Finish()

	mockJobStore := mock.NewMockJobStore(ctrl)
	mockRunnerStore := mock.NewMockRunnerStore(ctrl)
	mockScheduler := mock.NewMockScheduler(ctrl)
	h := New(mockJobStore, mockRunnerStore, mockScheduler)

	event := &github.WorkflowJobEvent{
		Action: github.Ptr("queued"),
		WorkflowJob: &github.WorkflowJob{
			ID: github.Ptr(int64(1001)),
		},
		Repo: &github.Repository{
			Name: github.Ptr("hello-world"),
			Owner: &github.User{
				Login: github.Ptr("octocat"),
			},
		},
		Installation: nil, // Missing
	}

	payload, err := json.Marshal(event)
	if err != nil {
		t.Fatal(err)
	}

	err = h.Handle(noContext, "workflow_job", "test-delivery", payload)
	if err == nil {
		t.Error("Expected error for missing installation")
	}
	if got := err.Error(); got != "missing installation in event payload" {
		t.Errorf("Want error message 'missing installation in event payload', got %q", got)
	}
}

func TestHandler_Handle_CreateError(t *testing.T) {
	ctrl := gomock.NewController(t)
	defer ctrl.Finish()

	mockJobStore := mock.NewMockJobStore(ctrl)
	mockRunnerStore := mock.NewMockRunnerStore(ctrl)
	mockScheduler := mock.NewMockScheduler(ctrl)
	h := New(mockJobStore, mockRunnerStore, mockScheduler)

	event := &github.WorkflowJobEvent{
		Action: github.Ptr("queued"),
		WorkflowJob: &github.WorkflowJob{
			ID:           github.Ptr(int64(1001)),
			RunID:        github.Ptr(int64(2001)),
			WorkflowName: github.Ptr("CI"),
			Status:       github.Ptr(core.JobStatusQueued),
			Labels:       []string{"cihub-2cpu-2048mb"},
		},
		Repo: &github.Repository{
			Name: github.Ptr("hello-world"),
			Owner: &github.User{
				Login: github.Ptr("octocat"),
			},
		},
		Installation: &github.Installation{
			ID: github.Ptr(int64(3001)),
		},
	}

	payload, err := json.Marshal(event)
	if err != nil {
		t.Fatal(err)
	}

	mockJobStore.EXPECT().
		Find(gomock.Any(), int64(1001)).
		Return(nil, errors.New("not found"))

	mockJobStore.EXPECT().
		Create(gomock.Any(), gomock.Any()).
		Return(errors.New("database error"))

	err = h.Handle(noContext, "workflow_job", "test-delivery", payload)
	if err == nil {
		t.Error("Expected error when Create fails")
	}
}

func TestHandler_Handle_UpdateError(t *testing.T) {
	ctrl := gomock.NewController(t)
	defer ctrl.Finish()

	mockJobStore := mock.NewMockJobStore(ctrl)
	mockRunnerStore := mock.NewMockRunnerStore(ctrl)
	mockScheduler := mock.NewMockScheduler(ctrl)
	h := New(mockJobStore, mockRunnerStore, mockScheduler)

	event := &github.WorkflowJobEvent{
		Action: github.Ptr("in_progress"),
		WorkflowJob: &github.WorkflowJob{
			ID:           github.Ptr(int64(1001)),
			RunID:        github.Ptr(int64(2001)),
			WorkflowName: github.Ptr("CI"),
			Status:       github.Ptr(core.JobStatusInProgress),
			Labels:       []string{"cihub-2cpu-2048mb"},
		},
		Repo: &github.Repository{
			Name: github.Ptr("hello-world"),
			Owner: &github.User{
				Login: github.Ptr("octocat"),
			},
		},
		Installation: &github.Installation{
			ID: github.Ptr(int64(3001)),
		},
	}

	payload, err := json.Marshal(event)
	if err != nil {
		t.Fatal(err)
	}

	existingJob := &core.Job{
		ID:      1001,
		Version: 1,
		Created: time.Now().Unix(),
	}

	mockJobStore.EXPECT().
		Find(gomock.Any(), int64(1001)).
		Return(existingJob, nil)

	mockJobStore.EXPECT().
		Update(gomock.Any(), gomock.Any()).
		Return(errors.New("optimistic lock error"))

	err = h.Handle(noContext, "workflow_job", "test-delivery", payload)
	if err == nil {
		t.Error("Expected error when Update fails")
	}
}

func TestConvertWorkflowJobToJob(t *testing.T) {
	createdAt := time.Unix(1234567890, 0)
	startedAt := time.Unix(1234567900, 0)
	completedAt := time.Unix(1234567950, 0)

	event := &github.WorkflowJobEvent{
		WorkflowJob: &github.WorkflowJob{
			ID:           github.Ptr(int64(1001)),
			RunID:        github.Ptr(int64(2001)),
			WorkflowName: github.Ptr("CI Pipeline"),
			Status:       github.Ptr(core.JobStatusCompleted),
			Conclusion:   github.Ptr("success"),
			HeadBranch:   github.Ptr("feature/test"),
			HeadSHA:      github.Ptr("def456"),
			Labels:       []string{"self-hosted", "linux", "x64"},
			RunnerID:     github.Ptr(int64(5001)),
			RunnerName:   github.Ptr("runner-awesome"),
			HTMLURL:      github.Ptr("https://github.com/octocat/github/actions/runs/18822693234/job/53737137650"),
			CreatedAt:    &github.Timestamp{Time: createdAt},
			StartedAt:    &github.Timestamp{Time: startedAt},
			CompletedAt:  &github.Timestamp{Time: completedAt},
		},
		Repo: &github.Repository{
			Name: github.Ptr("testrepo"),
			Owner: &github.User{
				Login: github.Ptr("testorg"),
			},
		},
		Installation: &github.Installation{
			ID: github.Ptr(int64(3001)),
		},
		Sender: &github.User{
			Login:     github.Ptr("octouser"),
			AvatarURL: github.Ptr("https://avatars.githubusercontent.com/u/1?v=4"),
		},
	}

	job := convertWorkflowJobToJob(event)

	tests := []struct {
		name string
		got  interface{}
		want interface{}
	}{
		{"ID", job.ID, int64(1001)},
		{"RunID", job.RunID, int64(2001)},
		{"InstallationID", job.InstallationID, int64(3001)},
		{"Owner", job.Owner, "testorg"},
		{"Repo", job.Repo, "testrepo"},
		{"Workflow", job.Workflow, "CI Pipeline"},
		{"Branch", job.Branch, "feature/test"},
		{"SHA", job.SHA, "def456"},
		{"Status", job.Status, core.JobStatusCompleted},
		{"Conclusion", job.Conclusion, "success"},
		{"RunnerID", job.RunnerID, int64(5001)},
		{"RunnerName", job.RunnerName, "runner-awesome"},
		{"URL", job.URL, "https://github.com/octocat/github/actions/runs/18822693234/job/53737137650"},
		{"AuthorLogin", job.AuthorLogin, "octouser"},
		{"AuthorAvatar", job.AuthorAvatar, "https://avatars.githubusercontent.com/u/1?v=4"},
		{"Queued", job.Queued, createdAt.Unix()},
		{"Started", job.Started, startedAt.Unix()},
		{"Completed", job.Completed, completedAt.Unix()},
		{"Labels", len(job.Labels), 3},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			if tt.got != tt.want {
				t.Errorf("Want %s = %v, got %v", tt.name, tt.want, tt.got)
			}
		})
	}

	// Verify labels content
	expectedLabels := []string{"self-hosted", "linux", "x64"}
	for i, label := range expectedLabels {
		if job.Labels[i] != label {
			t.Errorf("Want label[%d] = %s, got %s", i, label, job.Labels[i])
		}
	}
}

func TestConvertWorkflowJobToJob_EmptyTimestamps(t *testing.T) {
	event := &github.WorkflowJobEvent{
		WorkflowJob: &github.WorkflowJob{
			ID:           github.Ptr(int64(1001)),
			RunID:        github.Ptr(int64(2001)),
			WorkflowName: github.Ptr("CI"),
			Status:       github.Ptr(core.JobStatusQueued),
			// No timestamps set
		},
		Repo: &github.Repository{
			Name: github.Ptr("testrepo"),
			Owner: &github.User{
				Login: github.Ptr("testorg"),
			},
		},
		Installation: &github.Installation{
			ID: github.Ptr(int64(3001)),
		},
	}

	job := convertWorkflowJobToJob(event)

	// Verify zero timestamps are not converted
	if job.Queued != 0 {
		t.Errorf("Want Queued = 0 for missing CreatedAt, got %d", job.Queued)
	}
	if job.Started != 0 {
		t.Errorf("Want Started = 0 for missing StartedAt, got %d", job.Started)
	}
	if job.Completed != 0 {
		t.Errorf("Want Completed = 0 for missing CompletedAt, got %d", job.Completed)
	}
}

func TestConvertWorkflowJobToJob_WithAuthorInfo(t *testing.T) {
	event := &github.WorkflowJobEvent{
		WorkflowJob: &github.WorkflowJob{
			ID:           github.Ptr(int64(1001)),
			RunID:        github.Ptr(int64(2001)),
			WorkflowName: github.Ptr("Test Workflow"),
			Status:       github.Ptr(core.JobStatusQueued),
		},
		Repo: &github.Repository{
			Name: github.Ptr("testrepo"),
			Owner: &github.User{
				Login: github.Ptr("testorg"),
			},
		},
		Installation: &github.Installation{
			ID: github.Ptr(int64(3001)),
		},
		Sender: &github.User{
			Login:     github.Ptr("john-developer"),
			AvatarURL: github.Ptr("https://avatars.githubusercontent.com/u/12345?v=4"),
		},
	}

	job := convertWorkflowJobToJob(event)

	if got, want := job.AuthorLogin, "john-developer"; got != want {
		t.Errorf("Want AuthorLogin = %s, got %s", want, got)
	}
	if got, want := job.AuthorAvatar, "https://avatars.githubusercontent.com/u/12345?v=4"; got != want {
		t.Errorf("Want AuthorAvatar = %s, got %s", want, got)
	}
}

func TestConvertWorkflowJobToJob_NoSender(t *testing.T) {
	event := &github.WorkflowJobEvent{
		WorkflowJob: &github.WorkflowJob{
			ID:           github.Ptr(int64(1001)),
			RunID:        github.Ptr(int64(2001)),
			WorkflowName: github.Ptr("Test Workflow"),
			Status:       github.Ptr(core.JobStatusQueued),
		},
		Repo: &github.Repository{
			Name: github.Ptr("testrepo"),
			Owner: &github.User{
				Login: github.Ptr("testorg"),
			},
		},
		Installation: &github.Installation{
			ID: github.Ptr(int64(3001)),
		},
		Sender: nil, // No sender provided
	}

	job := convertWorkflowJobToJob(event)

	// Verify AuthorLogin and AuthorAvatar are empty when Sender is nil
	if got, want := job.AuthorLogin, ""; got != want {
		t.Errorf("Want empty AuthorLogin, got %s", got)
	}
	if got, want := job.AuthorAvatar, ""; got != want {
		t.Errorf("Want empty AuthorAvatar, got %s", got)
	}
}

func TestHandler_Handle_Waiting_WithAuthorInfo(t *testing.T) {
	ctrl := gomock.NewController(t)
	defer ctrl.Finish()

	mockJobStore := mock.NewMockJobStore(ctrl)
	mockRunnerStore := mock.NewMockRunnerStore(ctrl)
	mockScheduler := mock.NewMockScheduler(ctrl)
	h := New(mockJobStore, mockRunnerStore, mockScheduler)

	now := time.Now()
	event := &github.WorkflowJobEvent{
		Action: github.Ptr("waiting"),
		WorkflowJob: &github.WorkflowJob{
			ID:           github.Ptr(int64(1001)),
			RunID:        github.Ptr(int64(2001)),
			WorkflowName: github.Ptr("CI"),
			Status:       github.Ptr(core.JobStatusQueued),
			HeadBranch:   github.Ptr("main"),
			HeadSHA:      github.Ptr("abc123"),
			Labels:       []string{"cihub-2cpu-2048mb"},
			HTMLURL:      github.Ptr("https://github.com/octocat/hello-world/actions/runs/2001/jobs/1001"),
			CreatedAt:    &github.Timestamp{Time: now},
		},
		Repo: &github.Repository{
			Name: github.Ptr("hello-world"),
			Owner: &github.User{
				Login: github.Ptr("octocat"),
			},
		},
		Installation: &github.Installation{
			ID: github.Ptr(int64(3001)),
		},
		Sender: &github.User{
			Login:     github.Ptr("alice-dev"),
			AvatarURL: github.Ptr("https://avatars.githubusercontent.com/u/123?v=4"),
		},
	}

	payload, err := json.Marshal(event)
	if err != nil {
		t.Fatal(err)
	}

	// Expect Find to return error (job doesn't exist)
	mockJobStore.EXPECT().
		Find(gomock.Any(), int64(1001)).
		Return(nil, errors.New("not found"))

	// Expect Create to be called with author info preserved
	mockJobStore.EXPECT().
		Create(gomock.Any(), gomock.Any()).
		DoAndReturn(func(ctx context.Context, job *core.Job) error {
			// Verify author info is captured
			if got, want := job.AuthorLogin, "alice-dev"; got != want {
				t.Errorf("Want AuthorLogin %s, got %s", want, got)
			}
			if got, want := job.AuthorAvatar, "https://avatars.githubusercontent.com/u/123?v=4"; got != want {
				t.Errorf("Want AuthorAvatar %s, got %s", want, got)
			}
			return nil
		})

	if err := h.Handle(noContext, "workflow_job", "test-delivery", payload); err != nil {
		t.Errorf("Handle failed: %v", err)
	}
}

func TestHandler_Handle_Queued_UpdateWithAuthorInfo(t *testing.T) {
	ctrl := gomock.NewController(t)
	defer ctrl.Finish()

	mockJobStore := mock.NewMockJobStore(ctrl)
	mockRunnerStore := mock.NewMockRunnerStore(ctrl)
	mockScheduler := mock.NewMockScheduler(ctrl)
	h := New(mockJobStore, mockRunnerStore, mockScheduler)

	createdTime := time.Now().Add(-5 * time.Minute)
	now := time.Now()

	event := &github.WorkflowJobEvent{
		Action: github.Ptr("queued"),
		WorkflowJob: &github.WorkflowJob{
			ID:           github.Ptr(int64(1001)),
			RunID:        github.Ptr(int64(2001)),
			WorkflowName: github.Ptr("CI"),
			Status:       github.Ptr(core.JobStatusQueued),
			HeadBranch:   github.Ptr("main"),
			HeadSHA:      github.Ptr("abc123"),
			Labels:       []string{"cihub-2cpu-2048mb"},
			CreatedAt:    &github.Timestamp{Time: now},
		},
		Repo: &github.Repository{
			Name: github.Ptr("hello-world"),
			Owner: &github.User{
				Login: github.Ptr("octocat"),
			},
		},
		Installation: &github.Installation{
			ID: github.Ptr(int64(3001)),
		},
		Sender: &github.User{
			Login:     github.Ptr("bob-ci"),
			AvatarURL: github.Ptr("https://avatars.githubusercontent.com/u/456?v=4"),
		},
	}

	payload, err := json.Marshal(event)
	if err != nil {
		t.Fatal(err)
	}

	existingJob := &core.Job{
		ID:      1001,
		RunID:   2001,
		Status:  core.JobStatusQueued,
		Version: 2,
		Created: createdTime.Unix(),
		Updated: createdTime.Unix(),
	}

	// Expect Find to return existing job
	mockJobStore.EXPECT().
		Find(gomock.Any(), int64(1001)).
		Return(existingJob, nil)

	// Expect Update to be called with author info
	mockJobStore.EXPECT().
		Update(gomock.Any(), gomock.Any()).
		DoAndReturn(func(ctx context.Context, job *core.Job) error {
			// Verify author info is preserved in update
			if got, want := job.AuthorLogin, "bob-ci"; got != want {
				t.Errorf("Want AuthorLogin %s, got %s", want, got)
			}
			if got, want := job.AuthorAvatar, "https://avatars.githubusercontent.com/u/456?v=4"; got != want {
				t.Errorf("Want AuthorAvatar %s, got %s", want, got)
			}
			return nil
		})

	// Expect runner to be created
	mockRunnerStore.EXPECT().
		Create(gomock.Any(), gomock.Any()).
		Return(nil)

	// Expect Schedule to be called
	mockScheduler.EXPECT().
		Schedule(gomock.Any(), gomock.Any()).
		Return(nil)

	if err := h.Handle(noContext, "workflow_job", "test-delivery", payload); err != nil {
		t.Errorf("Handle failed: %v", err)
	}
}

func TestHandler_Handle_NoMatchingLabel_IgnoresEvent(t *testing.T) {
	ctrl := gomock.NewController(t)
	defer ctrl.Finish()

	mockJobStore := mock.NewMockJobStore(ctrl)
	mockRunnerStore := mock.NewMockRunnerStore(ctrl)
	mockScheduler := mock.NewMockScheduler(ctrl)
	h := New(mockJobStore, mockRunnerStore, mockScheduler)

	event := &github.WorkflowJobEvent{
		Action: github.Ptr("queued"),
		WorkflowJob: &github.WorkflowJob{
			ID:           github.Ptr(int64(1001)),
			RunID:        github.Ptr(int64(2001)),
			WorkflowName: github.Ptr("CI"),
			Status:       github.Ptr(core.JobStatusQueued),
			Labels:       []string{"self-hosted", "linux"}, // No cihub- label
		},
		Repo: &github.Repository{
			Name: github.Ptr("hello-world"),
			Owner: &github.User{
				Login: github.Ptr("octocat"),
			},
		},
		Installation: &github.Installation{
			ID: github.Ptr(int64(3001)),
		},
	}

	payload, err := json.Marshal(event)
	if err != nil {
		t.Fatal(err)
	}

	// Expect no calls to store or scheduler since no cihub- label exists
	// (no expectations set, will fail if any method is called)

	err = h.Handle(noContext, "workflow_job", "test-delivery", payload)
	if err != nil {
		t.Errorf("Handle should not fail when no cihub label found, got: %v", err)
	}
}

func TestHandler_Handle_MatchingLabel_ProcessesEvent(t *testing.T) {
	ctrl := gomock.NewController(t)
	defer ctrl.Finish()

	mockJobStore := mock.NewMockJobStore(ctrl)
	mockRunnerStore := mock.NewMockRunnerStore(ctrl)
	mockScheduler := mock.NewMockScheduler(ctrl)
	h := New(mockJobStore, mockRunnerStore, mockScheduler)

	event := &github.WorkflowJobEvent{
		Action: github.Ptr("queued"),
		WorkflowJob: &github.WorkflowJob{
			ID:           github.Ptr(int64(1001)),
			RunID:        github.Ptr(int64(2001)),
			WorkflowName: github.Ptr("CI"),
			Status:       github.Ptr(core.JobStatusQueued),
			Labels:       []string{"unknown-label", "cihub-2cpu-2048mb"}, // Has matching label
		},
		Repo: &github.Repository{
			Name: github.Ptr("hello-world"),
			Owner: &github.User{
				Login: github.Ptr("octocat"),
			},
		},
		Installation: &github.Installation{
			ID: github.Ptr(int64(3001)),
		},
	}

	payload, err := json.Marshal(event)
	if err != nil {
		t.Fatal(err)
	}

	// Expect Find to return error (job doesn't exist)
	mockJobStore.EXPECT().
		Find(gomock.Any(), int64(1001)).
		Return(nil, errors.New("not found"))

	// Expect Create to be called
	mockJobStore.EXPECT().
		Create(gomock.Any(), gomock.Any()).
		Return(nil)

	// Expect runner to be created
	mockRunnerStore.EXPECT().
		Create(gomock.Any(), gomock.Any()).
		Return(nil)

	// Expect Schedule to be called for queued action
	mockScheduler.EXPECT().
		Schedule(gomock.Any(), gomock.Any()).
		Return(nil)

	err = h.Handle(noContext, "workflow_job", "test-delivery", payload)
	if err != nil {
		t.Errorf("Handle should process event with matching label, got: %v", err)
	}
}

func TestHandler_ResolveJobSpecification(t *testing.T) {
	ctrl := gomock.NewController(t)
	defer ctrl.Finish()


	mockJobStore := mock.NewMockJobStore(ctrl)
	mockRunnerStore := mock.NewMockRunnerStore(ctrl)
	mockScheduler := mock.NewMockScheduler(ctrl)
	h := New( mockJobStore, mockRunnerStore, mockScheduler)

	createdTime := time.Now()
	event := &github.WorkflowJobEvent{
		Action: github.Ptr("queued"),
		WorkflowJob: &github.WorkflowJob{
			ID:           github.Ptr(int64(1001)),
			RunID:        github.Ptr(int64(2001)),
			WorkflowName: github.Ptr("CI"),
			Status:       github.Ptr(core.JobStatusQueued),
			Labels:       []string{"self-hosted", "cihub-2cpu-2048mb", "linux"},
			CreatedAt:    &github.Timestamp{Time: createdTime},
		},
		Repo: &github.Repository{
			Name: github.Ptr("hello-world"),
			Owner: &github.User{
				Login: github.Ptr("octocat"),
			},
		},
		Installation: &github.Installation{
			ID: github.Ptr(int64(3001)),
		},
	}

	payload, err := json.Marshal(event)
	if err != nil {
		t.Fatal(err)
	}

	// Expect Find to return error (job doesn't exist)
	mockJobStore.EXPECT().
		Find(gomock.Any(), int64(1001)).
		Return(nil, errors.New("not found"))

	// Expect Create to be called
	mockJobStore.EXPECT().
		Create(gomock.Any(), gomock.Any()).
		Return(nil)

	// Expect runner to be created with correct specification from matched label
	mockRunnerStore.EXPECT().
		Create(gomock.Any(), gomock.Any()).
		DoAndReturn(func(ctx context.Context, runner *core.Runner) error {
			// Verify runner specification was set from label
			if got, want := runner.Arch, "amd64"; got != want {
				t.Errorf("Want runner.Arch = %s, got %s", want, got)
			}
			if got, want := runner.RAM, int64(2048); got != want {
				t.Errorf("Want runner.RAM = %d, got %d", want, got)
			}
			if got, want := runner.CPU, int64(2); got != want {
				t.Errorf("Want runner.CPU = %d, got %d", want, got)
			}
			return nil
		})

	// Expect Schedule to be called for queued action
	mockScheduler.EXPECT().
		Schedule(gomock.Any(), gomock.Any()).
		Return(nil)

	if err := h.Handle(noContext, "workflow_job", "test-delivery", payload); err != nil {
		t.Errorf("Handle failed: %v", err)
	}
}

func TestHandler_ResolveJobSpecification_FirstMatchingLabel(t *testing.T) {
	ctrl := gomock.NewController(t)
	defer ctrl.Finish()


	mockJobStore := mock.NewMockJobStore(ctrl)
	mockRunnerStore := mock.NewMockRunnerStore(ctrl)
	mockScheduler := mock.NewMockScheduler(ctrl)
	h := New( mockJobStore, mockRunnerStore, mockScheduler)

	createdTime := time.Now()
	event := &github.WorkflowJobEvent{
		Action: github.Ptr("queued"),
		WorkflowJob: &github.WorkflowJob{
			ID:           github.Ptr(int64(1001)),
			RunID:        github.Ptr(int64(2001)),
			WorkflowName: github.Ptr("CI"),
			Status:       github.Ptr(core.JobStatusQueued),
			// Both labels are available, should match the first one
			Labels:    []string{"cihub-4cpu-4gb-arm64", "cihub-2cpu-2048mb"},
			CreatedAt: &github.Timestamp{Time: createdTime},
		},
		Repo: &github.Repository{
			Name: github.Ptr("hello-world"),
			Owner: &github.User{
				Login: github.Ptr("octocat"),
			},
		},
		Installation: &github.Installation{
			ID: github.Ptr(int64(3001)),
		},
	}

	payload, err := json.Marshal(event)
	if err != nil {
		t.Fatal(err)
	}

	// Expect Find to return error (job doesn't exist)
	mockJobStore.EXPECT().
		Find(gomock.Any(), int64(1001)).
		Return(nil, errors.New("not found"))

	// Expect Create to be called
	mockJobStore.EXPECT().
		Create(gomock.Any(), gomock.Any()).
		Return(nil)

	// Expect runner to be created with first matching label specification
	mockRunnerStore.EXPECT().
		Create(gomock.Any(), gomock.Any()).
		DoAndReturn(func(ctx context.Context, runner *core.Runner) error {
			// Should resolve to first matching label in job.Labels order
			if got, want := runner.Arch, "arm64"; got != want {
				t.Errorf("Want runner.Arch = %s, got %s", want, got)
			}
			if got, want := runner.RAM, int64(4096); got != want {
				t.Errorf("Want runner.RAM = %d, got %d", want, got)
			}
			if got, want := runner.CPU, int64(4); got != want {
				t.Errorf("Want runner.CPU = %d, got %d", want, got)
			}
			return nil
		})

	// Expect Schedule to be called for queued action
	mockScheduler.EXPECT().
		Schedule(gomock.Any(), gomock.Any()).
		Return(nil)

	if err := h.Handle(noContext, "workflow_job", "test-delivery", payload); err != nil {
		t.Errorf("Handle failed: %v", err)
	}
}
