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

// testLabels returns a sample Labels map for testing
func testLabels() core.Labels {
	return core.Labels{
		"cihub-2vcpu-amd64-ubuntu2404": {
			ID:     "cihub-2vcpu-amd64-ubuntu2404",
			Arch:   "amd64",
			OS:  "ubuntu2404",
			Memory: 2048,
			VCPU:   2,
		},
		"cihub-4vcpu-arm64-ubuntu2204": {
			ID:     "cihub-4vcpu-arm64-ubuntu2204",
			Arch:   "arm64",
			OS:  "ubuntu2204",
			Memory: 4096,
			VCPU:   4,
		},
	}
}

func TestHandler_Handles(t *testing.T) {
	ctrl := gomock.NewController(t)
	defer ctrl.Finish()

	mockJobStore := mock.NewMockJobStore(ctrl)
	mockRunnerStore := mock.NewMockRunnerStore(ctrl)
	mockScheduler := mock.NewMockScheduler(ctrl)
	h := New(testLabels(), mockJobStore, mockRunnerStore, mockScheduler)

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
	h := New(testLabels(), mockJobStore, mockRunnerStore, mockScheduler)

	now := time.Now()
	event := &github.WorkflowJobEvent{
		Action: github.String("waiting"),
		WorkflowJob: &github.WorkflowJob{
			ID:           github.Int64(1001),
			RunID:        github.Int64(2001),
			WorkflowName: github.String("CI"),
			Status:       github.String(core.JobStatusQueued),
			HeadBranch:   github.String("main"),
			HeadSHA:      github.String("abc123"),
			Labels:       []string{"cihub-2vcpu-amd64-ubuntu2404"},
			URL:          github.String("https://api.github.com/repos/octocat/hello-world/actions/jobs/1001"),
			CreatedAt:    &github.Timestamp{Time: now},
		},
		Repo: &github.Repository{
			Name: github.String("hello-world"),
			Owner: &github.User{
				Login: github.String("octocat"),
			},
		},
		Installation: &github.Installation{
			ID: github.Int64(3001),
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
	h := New(testLabels(), mockJobStore, mockRunnerStore, mockScheduler)

	event := &github.WorkflowJobEvent{
		Action: github.String("waiting"),
		WorkflowJob: &github.WorkflowJob{
			ID:           github.Int64(1001),
			RunID:        github.Int64(2001),
			WorkflowName: github.String("CI"),
			Status:       github.String(core.JobStatusQueued),
			Labels:       []string{"cihub-2vcpu-amd64-ubuntu2404"},
		},
		Repo: &github.Repository{
			Name: github.String("hello-world"),
			Owner: &github.User{
				Login: github.String("octocat"),
			},
		},
		Installation: &github.Installation{
			ID: github.Int64(3001),
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
	h := New(testLabels(), mockJobStore, mockRunnerStore, mockScheduler)

	now := time.Now()
	event := &github.WorkflowJobEvent{
		Action: github.String("queued"),
		WorkflowJob: &github.WorkflowJob{
			ID:           github.Int64(1001),
			RunID:        github.Int64(2001),
			WorkflowName: github.String("CI"),
			Status:       github.String(core.JobStatusQueued),
			HeadBranch:   github.String("main"),
			HeadSHA:      github.String("abc123"),
			Labels:       []string{"cihub-2vcpu-amd64-ubuntu2404"},
			URL:          github.String("https://api.github.com/repos/octocat/hello-world/actions/jobs/1001"),
			CreatedAt:    &github.Timestamp{Time: now},
		},
		Repo: &github.Repository{
			Name: github.String("hello-world"),
			Owner: &github.User{
				Login: github.String("octocat"),
			},
		},
		Installation: &github.Installation{
			ID: github.Int64(3001),
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
	h := New(testLabels(), mockJobStore, mockRunnerStore, mockScheduler)

	createdTime := time.Now().Add(-5 * time.Minute)
	now := time.Now()

	event := &github.WorkflowJobEvent{
		Action: github.String("queued"),
		WorkflowJob: &github.WorkflowJob{
			ID:           github.Int64(1001),
			RunID:        github.Int64(2001),
			WorkflowName: github.String("CI"),
			Status:       github.String(core.JobStatusQueued),
			HeadBranch:   github.String("main"),
			HeadSHA:      github.String("abc123"),
			Labels:       []string{"cihub-2vcpu-amd64-ubuntu2404"},
			CreatedAt:    &github.Timestamp{Time: now},
		},
		Repo: &github.Repository{
			Name: github.String("hello-world"),
			Owner: &github.User{
				Login: github.String("octocat"),
			},
		},
		Installation: &github.Installation{
			ID: github.Int64(3001),
		},
	}

	payload, err := json.Marshal(event)
	if err != nil {
		t.Fatal(err)
	}

	existingJob := &core.Job{
		ID:       1001,
		RunID:    2001,
		Status:   core.JobStatusQueued,
		Version:  2,
		Created:  createdTime.Unix(),
		Updated:  createdTime.Unix(),
		Machine:  "node-1",
		Accepted: createdTime.Unix(),
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
			if got, want := job.Machine, "node-1"; got != want {
				t.Errorf("Want machine %s preserved, got %s", want, got)
			}
			if got, want := job.Accepted, createdTime.Unix(); got != want {
				t.Errorf("Want accepted %d preserved, got %d", want, got)
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

func TestHandler_Handle_InProgress_UpdateExisting(t *testing.T) {
	ctrl := gomock.NewController(t)
	defer ctrl.Finish()

	mockJobStore := mock.NewMockJobStore(ctrl)
	mockRunnerStore := mock.NewMockRunnerStore(ctrl)
	mockScheduler := mock.NewMockScheduler(ctrl)
	h := New(testLabels(), mockJobStore, mockRunnerStore, mockScheduler)

	createdTime := time.Now().Add(-5 * time.Minute)
	startedTime := time.Now()

	event := &github.WorkflowJobEvent{
		Action: github.String("in_progress"),
		WorkflowJob: &github.WorkflowJob{
			ID:           github.Int64(1001),
			RunID:        github.Int64(2001),
			WorkflowName: github.String("CI"),
			Status:       github.String(core.JobStatusInProgress),
			HeadBranch:   github.String("main"),
			HeadSHA:      github.String("abc123"),
			Labels:       []string{"cihub-2vcpu-amd64-ubuntu2404"},
			RunnerID:     github.Int64(5001),
			RunnerName:   github.String("runner-1"),
			URL:          github.String("https://api.github.com/repos/octocat/hello-world/actions/jobs/1001"),
			CreatedAt:    &github.Timestamp{Time: createdTime},
			StartedAt:    &github.Timestamp{Time: startedTime},
		},
		Repo: &github.Repository{
			Name: github.String("hello-world"),
			Owner: &github.User{
				Login: github.String("octocat"),
			},
		},
		Installation: &github.Installation{
			ID: github.Int64(3001),
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
		Machine:        "node-1",
		Accepted:       startedTime.Unix() - 10,
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
			if got, want := job.Machine, "node-1"; got != want {
				t.Errorf("Want machine %s preserved, got %s", want, got)
			}
			if got, want := job.Accepted, startedTime.Unix()-10; got != want {
				t.Errorf("Want accepted timestamp %d preserved, got %d", want, got)
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
			Name:       "runner-1",
			ID:         5001,
			Status:     core.RunnerStatusIdle,
			AssignedTo: 0,
			Busy:       false,
		}, nil)

	// Expect runner update to mark it as busy
	mockRunnerStore.EXPECT().
		Update(gomock.Any(), gomock.Any()).
		DoAndReturn(func(ctx context.Context, runner *core.Runner) error {
			if !runner.Busy {
				t.Error("Expected runner to be marked as busy")
			}
			if runner.Status != core.RunnerStatusBusy {
				t.Errorf("Expected runner status to be busy, got %s", runner.Status)
			}
			if runner.AssignedTo != 1001 {
				t.Errorf("Expected runner to be assigned to job 1001, got %d", runner.AssignedTo)
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
	h := New(testLabels(), mockJobStore, mockRunnerStore, mockScheduler)

	createdTime := time.Now().Add(-10 * time.Minute)
	startedTime := time.Now().Add(-5 * time.Minute)
	completedTime := time.Now()

	event := &github.WorkflowJobEvent{
		Action: github.String("completed"),
		WorkflowJob: &github.WorkflowJob{
			ID:           github.Int64(1001),
			RunID:        github.Int64(2001),
			WorkflowName: github.String("CI"),
			Status:       github.String(core.JobStatusCompleted),
			Conclusion:   github.String("success"),
			HeadBranch:   github.String("main"),
			HeadSHA:      github.String("abc123"),
			Labels:       []string{"cihub-2vcpu-amd64-ubuntu2404"},
			RunnerID:     github.Int64(5001),
			RunnerName:   github.String("runner-1"),
			URL:          github.String("https://api.github.com/repos/octocat/hello-world/actions/jobs/1001"),
			CreatedAt:    &github.Timestamp{Time: createdTime},
			StartedAt:    &github.Timestamp{Time: startedTime},
			CompletedAt:  &github.Timestamp{Time: completedTime},
		},
		Repo: &github.Repository{
			Name: github.String("hello-world"),
			Owner: &github.User{
				Login: github.String("octocat"),
			},
		},
		Installation: &github.Installation{
			ID: github.Int64(3001),
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
			Name:       "runner-1",
			ID:         5001,
			Status:     core.RunnerStatusBusy,
			AssignedTo: 1001,
			Busy:       true,
		}, nil)

	// Expect Update to mark runner as completed
	mockRunnerStore.EXPECT().
		Update(gomock.Any(), gomock.Any()).
		DoAndReturn(func(ctx context.Context, runner *core.Runner) error {
			if runner.Busy {
				t.Error("Expected runner to be marked as not busy")
			}
			if runner.Status != core.RunnerStatusCompleted {
				t.Errorf("Expected runner status to be completed, got %s", runner.Status)
			}
			if runner.AssignedTo != 1001 {
				t.Errorf("Expected runner to remain assigned to job 1001, got %d", runner.AssignedTo)
			}
			if runner.Completed == 0 {
				t.Error("Expected runner completed timestamp to be set")
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
	h := New(testLabels(), mockJobStore, mockRunnerStore, mockScheduler)

	createdTime := time.Now().Add(-10 * time.Minute)
	completedTime := time.Now()

	event := &github.WorkflowJobEvent{
		Action: github.String("completed"),
		WorkflowJob: &github.WorkflowJob{
			ID:           github.Int64(1001),
			RunID:        github.Int64(2001),
			WorkflowName: github.String("CI"),
			Status:       github.String(core.JobStatusCompleted),
			Labels:       []string{"cihub-2vcpu-amd64-ubuntu2404"},
			RunnerName:   github.String("runner-1"),
			CreatedAt:    &github.Timestamp{Time: createdTime},
			CompletedAt:  &github.Timestamp{Time: completedTime},
		},
		Repo: &github.Repository{
			Name: github.String("hello-world"),
			Owner: &github.User{
				Login: github.String("octocat"),
			},
		},
		Installation: &github.Installation{
			ID: github.Int64(3001),
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
	h := New(testLabels(), mockJobStore, mockRunnerStore, mockScheduler)

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
	h := New(testLabels(), mockJobStore, mockRunnerStore, mockScheduler)

	event := &github.WorkflowJobEvent{
		Action:      github.String("queued"),
		WorkflowJob: nil, // Missing
		Repo: &github.Repository{
			Name: github.String("hello-world"),
			Owner: &github.User{
				Login: github.String("octocat"),
			},
		},
		Installation: &github.Installation{
			ID: github.Int64(3001),
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
	h := New(testLabels(), mockJobStore, mockRunnerStore, mockScheduler)

	event := &github.WorkflowJobEvent{
		Action: github.String("queued"),
		WorkflowJob: &github.WorkflowJob{
			ID: github.Int64(1001),
		},
		Repo: nil, // Missing
		Installation: &github.Installation{
			ID: github.Int64(3001),
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
	h := New(testLabels(), mockJobStore, mockRunnerStore, mockScheduler)

	event := &github.WorkflowJobEvent{
		Action: github.String("queued"),
		WorkflowJob: &github.WorkflowJob{
			ID: github.Int64(1001),
		},
		Repo: &github.Repository{
			Name: github.String("hello-world"),
			Owner: &github.User{
				Login: github.String("octocat"),
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
	h := New(testLabels(), mockJobStore, mockRunnerStore, mockScheduler)

	event := &github.WorkflowJobEvent{
		Action: github.String("queued"),
		WorkflowJob: &github.WorkflowJob{
			ID:           github.Int64(1001),
			RunID:        github.Int64(2001),
			WorkflowName: github.String("CI"),
			Status:       github.String(core.JobStatusQueued),
			Labels:       []string{"cihub-2vcpu-amd64-ubuntu2404"},
		},
		Repo: &github.Repository{
			Name: github.String("hello-world"),
			Owner: &github.User{
				Login: github.String("octocat"),
			},
		},
		Installation: &github.Installation{
			ID: github.Int64(3001),
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
	h := New(testLabels(), mockJobStore, mockRunnerStore, mockScheduler)

	event := &github.WorkflowJobEvent{
		Action: github.String("in_progress"),
		WorkflowJob: &github.WorkflowJob{
			ID:           github.Int64(1001),
			RunID:        github.Int64(2001),
			WorkflowName: github.String("CI"),
			Status:       github.String(core.JobStatusInProgress),
			Labels:       []string{"cihub-2vcpu-amd64-ubuntu2404"},
		},
		Repo: &github.Repository{
			Name: github.String("hello-world"),
			Owner: &github.User{
				Login: github.String("octocat"),
			},
		},
		Installation: &github.Installation{
			ID: github.Int64(3001),
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
			ID:           github.Int64(1001),
			RunID:        github.Int64(2001),
			WorkflowName: github.String("CI Pipeline"),
			Status:       github.String(core.JobStatusCompleted),
			Conclusion:   github.String("success"),
			HeadBranch:   github.String("feature/test"),
			HeadSHA:      github.String("def456"),
			Labels:       []string{"self-hosted", "linux", "x64"},
			RunnerID:     github.Int64(5001),
			RunnerName:   github.String("runner-awesome"),
			URL:          github.String("https://api.github.com/repos/testorg/testrepo/actions/jobs/1001"),
			CreatedAt:    &github.Timestamp{Time: createdAt},
			StartedAt:    &github.Timestamp{Time: startedAt},
			CompletedAt:  &github.Timestamp{Time: completedAt},
		},
		Repo: &github.Repository{
			Name: github.String("testrepo"),
			Owner: &github.User{
				Login: github.String("testorg"),
			},
		},
		Installation: &github.Installation{
			ID: github.Int64(3001),
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
		{"URL", job.URL, "https://api.github.com/repos/testorg/testrepo/actions/jobs/1001"},
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
			ID:           github.Int64(1001),
			RunID:        github.Int64(2001),
			WorkflowName: github.String("CI"),
			Status:       github.String(core.JobStatusQueued),
			// No timestamps set
		},
		Repo: &github.Repository{
			Name: github.String("testrepo"),
			Owner: &github.User{
				Login: github.String("testorg"),
			},
		},
		Installation: &github.Installation{
			ID: github.Int64(3001),
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

func TestHandler_Handle_NoMatchingLabel_IgnoresEvent(t *testing.T) {
	ctrl := gomock.NewController(t)
	defer ctrl.Finish()

	// Create labels without "cihub-4vcpu-arm64-ubuntu2204"
	labels := core.Labels{
		"cihub-2vcpu-amd64-ubuntu2404": {
			ID:     "cihub-2vcpu-amd64-ubuntu2404",
			Arch:   "amd64",
			OS:  "ubuntu2404",
			Memory: 2048,
			VCPU:   2,
		},
	}

	mockJobStore := mock.NewMockJobStore(ctrl)
	mockRunnerStore := mock.NewMockRunnerStore(ctrl)
	mockScheduler := mock.NewMockScheduler(ctrl)
	h := New(labels, mockJobStore, mockRunnerStore, mockScheduler)

	event := &github.WorkflowJobEvent{
		Action: github.String("queued"),
		WorkflowJob: &github.WorkflowJob{
			ID:           github.Int64(1001),
			RunID:        github.Int64(2001),
			WorkflowName: github.String("CI"),
			Status:       github.String(core.JobStatusQueued),
			Labels:       []string{"cihub-4vcpu-arm64-ubuntu2204"}, // Not in supported labels
		},
		Repo: &github.Repository{
			Name: github.String("hello-world"),
			Owner: &github.User{
				Login: github.String("octocat"),
			},
		},
		Installation: &github.Installation{
			ID: github.Int64(3001),
		},
	}

	payload, err := json.Marshal(event)
	if err != nil {
		t.Fatal(err)
	}

	// Expect no calls to store or scheduler
	// (no expectations set, will fail if any method is called)

	err = h.Handle(noContext, "workflow_job", "test-delivery", payload)
	if err != nil {
		t.Errorf("Handle should not fail for unsupported label, got: %v", err)
	}
}

func TestHandler_Handle_MatchingLabel_ProcessesEvent(t *testing.T) {
	ctrl := gomock.NewController(t)
	defer ctrl.Finish()

	labels := core.Labels{
		"cihub-2vcpu-amd64-ubuntu2404": {
			ID:     "cihub-2vcpu-amd64-ubuntu2404",
			Arch:   "amd64",
			OS:  "ubuntu2404",
			Memory: 2048,
			VCPU:   2,
		},
		"cihub-4vcpu-arm64-ubuntu2204": {
			ID:     "cihub-4vcpu-arm64-ubuntu2204",
			Arch:   "arm64",
			OS:  "ubuntu2204",
			Memory: 4096,
			VCPU:   4,
		},
	}

	mockJobStore := mock.NewMockJobStore(ctrl)
	mockRunnerStore := mock.NewMockRunnerStore(ctrl)
	mockScheduler := mock.NewMockScheduler(ctrl)
	h := New(labels, mockJobStore, mockRunnerStore, mockScheduler)

	event := &github.WorkflowJobEvent{
		Action: github.String("queued"),
		WorkflowJob: &github.WorkflowJob{
			ID:           github.Int64(1001),
			RunID:        github.Int64(2001),
			WorkflowName: github.String("CI"),
			Status:       github.String(core.JobStatusQueued),
			Labels:       []string{"unknown-label", "cihub-2vcpu-amd64-ubuntu2404"}, // Has matching label
		},
		Repo: &github.Repository{
			Name: github.String("hello-world"),
			Owner: &github.User{
				Login: github.String("octocat"),
			},
		},
		Installation: &github.Installation{
			ID: github.Int64(3001),
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

	labels := core.Labels{
		"cihub-2vcpu-amd64-ubuntu2404": {
			ID:     "cihub-2vcpu-amd64-ubuntu2404",
			Arch:   "amd64",
			OS:     "ghcr.io/getcihub/runner-ubuntu24.04:latest",
			Memory: 2048,
			VCPU:   2,
		},
		"cihub-4vcpu-arm64-ubuntu2204": {
			ID:     "cihub-4vcpu-arm64-ubuntu2204",
			Arch:   "arm64",
			OS:     "ghcr.io/getcihub/runner-ubuntu22.04:latest",
			Memory: 4096,
			VCPU:   4,
		},
	}

	mockJobStore := mock.NewMockJobStore(ctrl)
	mockRunnerStore := mock.NewMockRunnerStore(ctrl)
	mockScheduler := mock.NewMockScheduler(ctrl)
	h := New(labels, mockJobStore, mockRunnerStore, mockScheduler)

	createdTime := time.Now()
	event := &github.WorkflowJobEvent{
		Action: github.String("queued"),
		WorkflowJob: &github.WorkflowJob{
			ID:           github.Int64(1001),
			RunID:        github.Int64(2001),
			WorkflowName: github.String("CI"),
			Status:       github.String(core.JobStatusQueued),
			Labels:       []string{"self-hosted", "cihub-2vcpu-amd64-ubuntu2404", "linux"},
			CreatedAt:    &github.Timestamp{Time: createdTime},
		},
		Repo: &github.Repository{
			Name: github.String("hello-world"),
			Owner: &github.User{
				Login: github.String("octocat"),
			},
		},
		Installation: &github.Installation{
			ID: github.Int64(3001),
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

	// Expect Create to be called, verify job specification is set
	mockJobStore.EXPECT().
		Create(gomock.Any(), gomock.Any()).
		DoAndReturn(func(ctx context.Context, job *core.Job) error {
			// Verify job specification was resolved
			if got, want := job.OS, "ghcr.io/getcihub/runner-ubuntu24.04:latest"; got != want {
				t.Errorf("Want job.OS = %s, got %s", want, got)
			}
			if got, want := job.Arch, "amd64"; got != want {
				t.Errorf("Want job.Arch = %s, got %s", want, got)
			}
			if got, want := job.Memory, int64(2048); got != want {
				t.Errorf("Want job.Memory = %d, got %d", want, got)
			}
			if got, want := job.VCPU, int64(2); got != want {
				t.Errorf("Want job.VCPU = %d, got %d", want, got)
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

	labels := core.Labels{
		"cihub-2vcpu-amd64-ubuntu2404": {
			ID:     "cihub-2vcpu-amd64-ubuntu2404",
			Arch:   "amd64",
			OS:     "ghcr.io/getcihub/runner-ubuntu24.04:latest",
			Memory: 2048,
			VCPU:   2,
		},
		"cihub-4vcpu-arm64-ubuntu2204": {
			ID:     "cihub-4vcpu-arm64-ubuntu2204",
			Arch:   "arm64",
			OS:     "ghcr.io/getcihub/runner-ubuntu22.04:latest",
			Memory: 4096,
			VCPU:   4,
		},
	}

	mockJobStore := mock.NewMockJobStore(ctrl)
	mockRunnerStore := mock.NewMockRunnerStore(ctrl)
	mockScheduler := mock.NewMockScheduler(ctrl)
	h := New(labels, mockJobStore, mockRunnerStore, mockScheduler)

	createdTime := time.Now()
	event := &github.WorkflowJobEvent{
		Action: github.String("queued"),
		WorkflowJob: &github.WorkflowJob{
			ID:           github.Int64(1001),
			RunID:        github.Int64(2001),
			WorkflowName: github.String("CI"),
			Status:       github.String(core.JobStatusQueued),
			// Both labels are available, should match the first one
			Labels:    []string{"cihub-4vcpu-arm64-ubuntu2204", "cihub-2vcpu-amd64-ubuntu2404"},
			CreatedAt: &github.Timestamp{Time: createdTime},
		},
		Repo: &github.Repository{
			Name: github.String("hello-world"),
			Owner: &github.User{
				Login: github.String("octocat"),
			},
		},
		Installation: &github.Installation{
			ID: github.Int64(3001),
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

	// Expect Create to be called, verify first matching label is used
	mockJobStore.EXPECT().
		Create(gomock.Any(), gomock.Any()).
		DoAndReturn(func(ctx context.Context, job *core.Job) error {
			// Should resolve to first matching label in job.Labels order
			if got, want := job.OS, "ghcr.io/getcihub/runner-ubuntu22.04:latest"; got != want {
				t.Errorf("Want job.OS = %s (4vcpu), got %s", want, got)
			}
			if got, want := job.Arch, "arm64"; got != want {
				t.Errorf("Want job.Arch = %s, got %s", want, got)
			}
			if got, want := job.Memory, int64(4096); got != want {
				t.Errorf("Want job.Memory = %d, got %d", want, got)
			}
			if got, want := job.VCPU, int64(4); got != want {
				t.Errorf("Want job.VCPU = %d, got %d", want, got)
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
