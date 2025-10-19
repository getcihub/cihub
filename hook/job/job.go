package job

import (
	"context"
	"encoding/json"
	"time"

	"github.com/google/go-github/v76/github"
	"github.com/palantir/go-githubapp/githubapp"
	"github.com/pkg/errors"

	"github.com/getcihub/cihub/core"
)

type handler struct {
	jobs core.JobStore
}

func New(jobs core.JobStore) githubapp.EventHandler {
	return &handler{jobs}
}

func (h *handler) Handles() []string {
	return []string{"workflow_job"}
}

func (h *handler) Handle(ctx context.Context, eventType, deliveryID string, payload []byte) error {
	// Parse the GitHub webhook payload
	var event github.WorkflowJobEvent
	if err := json.Unmarshal(payload, &event); err != nil {
		return errors.Wrap(err, "failed to parse workflow job event payload")
	}

	// Validate required fields from the webhook
	if event.WorkflowJob == nil {
		return errors.New("missing workflow_job in event payload")
	}
	if event.Repo == nil {
		return errors.New("missing repository in event payload")
	}
	if event.Installation == nil {
		return errors.New("missing installation in event payload")
	}

	// Convert GitHub event to core.Job model
	job := convertWorkflowJobToJob(&event)

	// Attempt to find existing job for idempotency
	existing, err := h.jobs.Find(ctx, job.ID)
	if err != nil {
		// Job doesn't exist - create new record
		now := time.Now().Unix()
		job.Created = now
		job.Updated = now
		return errors.Wrap(h.jobs.Create(ctx, job), "failed to create job")
	}

	// Job exists - update with latest webhook data
	// Preserve version for optimistic locking
	job.Version = existing.Version
	job.Created = existing.Created
	job.Updated = time.Now().Unix()

	// Preserve agent-assigned fields that webhooks don't provide
	job.Machine = existing.Machine
	job.Accepted = existing.Accepted

	return errors.Wrap(h.jobs.Update(ctx, job), "failed to update job")
}

// convertWorkflowJobToJob converts a GitHub WorkflowJobEvent to a core.Job.
// It extracts relevant fields from the webhook payload and maps them to
// the internal job representation.
func convertWorkflowJobToJob(event *github.WorkflowJobEvent) *core.Job {
	wj := event.WorkflowJob
	repo := event.Repo
	installation := event.Installation

	job := &core.Job{
		ID:             wj.GetID(),
		RunID:          wj.GetRunID(),
		InstallationID: installation.GetID(),
		Owner:          repo.GetOwner().GetLogin(),
		Repo:           repo.GetName(),
		Workflow:       wj.GetWorkflowName(),
		Name:           wj.GetName(),
		Branch:         wj.GetHeadBranch(),
		SHA:            wj.GetHeadSHA(),
		Status:         wj.GetStatus(),
		Conclusion:     wj.GetConclusion(),
		Labels:         wj.Labels,
		RunnerID:       wj.GetRunnerID(),
		RunnerName:     wj.GetRunnerName(),
		URL:            wj.GetURL(),
	}

	// Convert GitHub timestamps to Unix timestamps
	if createdAt := wj.GetCreatedAt(); !createdAt.IsZero() {
		job.Queued = createdAt.Unix()
	}
	if startedAt := wj.GetStartedAt(); !startedAt.IsZero() {
		job.Started = startedAt.Unix()
	}
	if completedAt := wj.GetCompletedAt(); !completedAt.IsZero() {
		job.Completed = completedAt.Unix()
	}

	return job
}
