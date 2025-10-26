package job

import (
	"context"
	"encoding/json"
	"fmt"
	"time"

	"github.com/google/go-github/v76/github"
	"github.com/palantir/go-githubapp/githubapp"
	"github.com/pkg/errors"
	"github.com/sirupsen/logrus"

	"github.com/getcihub/cihub/core"
	"github.com/getcihub/cihub/logger"
)

type handler struct {
	labels    core.Labels
	jobs      core.JobStore
	runners   core.RunnerStore
	scheduler core.Scheduler
}

func New(
	labels core.Labels,
	jobs core.JobStore,
	runners core.RunnerStore,
	scheduler core.Scheduler,
) githubapp.EventHandler {
	return &handler{
		labels:    labels,
		jobs:      jobs,
		runners:   runners,
		scheduler: scheduler,
	}
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

	// Extract and validate action
	action := event.GetAction()
	if action == "" {
		return errors.New("missing action in workflow_job event")
	}

	// Convert GitHub event to core.Job model
	job := convertWorkflowJobToJob(&event)

	log := logger.FromContext(ctx).WithFields(
		logrus.Fields{
			"event":       eventType,
			"delivery.id": deliveryID,
			"action":      action,
			"job.id":      job.ID,
			"job.labels":  job.Labels,
			"run.id":      job.RunID,
			"owner":       job.Owner,
			"repo":        job.Repo,
			"status":      job.Status,
			"runner.id":   job.RunnerID,
			"runner.name": job.RunnerName,
		},
	)
	log.Infoln("hook: received workflow job event")

	// Check if job has a supported label
	if !h.labels.Has(job.Labels) {
		log.Debugln("hook: no matching label, ignore event")
		return nil
	}

	// Resolve job specification from matching label
	if err := h.resolveJobSpecification(job, log); err != nil {
		return err
	}

	// Route to action-specific handler
	switch action {
	case "waiting":
		return h.handleWaiting(ctx, log, job)
	case "queued":
		return h.handleQueued(ctx, log, job)
	case "in_progress":
		return h.handleInProgress(ctx, log, job)
	case "completed":
		return h.handleCompleted(ctx, log, job)
	default:
		log.Warnf("hook: unknown workflow job action: %s", action)
		return nil
	}
}

func (h *handler) handleWaiting(ctx context.Context, log *logrus.Entry, job *core.Job) error {
	// Try to find existing job for idempotency
	_, err := h.jobs.Find(ctx, job.ID)
	if err == nil {
		// Job already exists, nothing to do
		log.Debugln("hook: job already exists in waiting state")
		return nil
	}

	// Job doesn't exist, create new record
	now := time.Now().Unix()
	job.Created = now
	job.Updated = now

	if err := h.jobs.Create(ctx, job); err != nil {
		return fmt.Errorf("hook: failed to create job in waiting state, err: %w", err)
	}

	log.Infoln("hook: created job in waiting state")
	return nil
}

func (h *handler) handleQueued(ctx context.Context, log *logrus.Entry, job *core.Job) error {
	// Try to find existing job
	existing, err := h.jobs.Find(ctx, job.ID)
	if err != nil {
		// Create new job record
		now := time.Now().Unix()
		job.Created = now
		job.Updated = now

		if err := h.jobs.Create(ctx, job); err != nil {
			return fmt.Errorf("hook: failed to create queued job, err: %w", err)
		}

		log.Infoln("hook: created queued job")
		return h.scheduler.Schedule(ctx, job)
	}

	// Update existing job
	job.Version = existing.Version
	job.Created = existing.Created
	job.Updated = time.Now().Unix()

	// Preserve agent-assigned fields
	job.Machine = existing.Machine
	job.Accepted = existing.Accepted

	if err := h.jobs.Update(ctx, job); err != nil {
		return fmt.Errorf("hook: failed to update queued job, err: %w", err)
	}

	log.Infoln("hook: updated queued job")
	return h.scheduler.Schedule(ctx, job)
}

func (h *handler) handleInProgress(ctx context.Context, log *logrus.Entry, job *core.Job) error {
	// Try to find existing job
	existing, err := h.jobs.Find(ctx, job.ID)
	if err != nil {
		// Create new job record
		now := time.Now().Unix()
		job.Created = now
		job.Updated = now

		if err := h.jobs.Create(ctx, job); err != nil {
			return fmt.Errorf("hook: failed to create in_progress job, err: %w", err)
		}

		log.Infoln("hook: created in_progress job")
	} else {
		// Update existing job
		job.Version = existing.Version
		job.Created = existing.Created
		job.Updated = time.Now().Unix()

		// Preserve agent-assigned fields
		job.Machine = existing.Machine
		job.Accepted = existing.Accepted

		if err := h.jobs.Update(ctx, job); err != nil {
			return fmt.Errorf("hook: failed to update in_progress job, err: %w", err)
		}

		log.Infoln("hook: updated in_progress job")
	}

	// Sync runner if assigned
	if job.RunnerName != "" {
		if err := h.syncRunnerInProgress(ctx, log, job); err != nil {
			log.WithError(err).Warnln("hook: failed to sync runner")
			// Don't fail the entire handler if runner sync fails
		}
	}

	return nil
}

func (h *handler) handleCompleted(ctx context.Context, log *logrus.Entry, job *core.Job) error {
	// Try to find existing job
	existing, err := h.jobs.Find(ctx, job.ID)
	if err != nil {
		// Create new job record (shouldn't normally happen, but be safe)
		now := time.Now().Unix()
		job.Created = now
		job.Updated = now

		if err := h.jobs.Create(ctx, job); err != nil {
			return fmt.Errorf("hook: failed to create completed job, err: %w", err)
		}

		log.Infoln("hook: created completed job")
	} else {
		// Update existing job
		job.Version = existing.Version
		job.Created = existing.Created
		job.Updated = time.Now().Unix()

		// Preserve agent-assigned fields
		job.Machine = existing.Machine
		job.Accepted = existing.Accepted

		if err := h.jobs.Update(ctx, job); err != nil {
			return fmt.Errorf("hook: failed to update completed job, err: %w", err)
		}

		log.Infoln("hook: updated completed job")
	}

	// Sync runner if assigned
	if job.RunnerName != "" {
		if err := h.syncRunnerCompleted(ctx, log, job); err != nil {
			log.WithError(err).Warnln("hook: failed to sync runner")
			// Don't fail the entire handler if runner sync fails
		}
	}

	return nil
}

func (h *handler) syncRunnerInProgress(ctx context.Context, log *logrus.Entry, job *core.Job) error {
	runner, err := h.runners.Find(ctx, job.RunnerName)
	if err != nil {
		log.WithError(err).Warnf("hook: runner '%s' not found in datastore", job.RunnerName)
		return nil // Non-fatal: log and continue
	}

	runner.AssignedTo = job.ID
	runner.Busy = true
	runner.Status = core.RunnerStatusBusy
	runner.Updated = time.Now().Unix()

	if err := h.runners.Update(ctx, runner); err != nil {
		return fmt.Errorf("failed to update runner '%s': %w", job.RunnerName, err)
	}

	log.Infof("hook: synced runner '%s' to busy", job.RunnerName)
	return nil
}

func (h *handler) syncRunnerCompleted(ctx context.Context, log *logrus.Entry, job *core.Job) error {
	runner, err := h.runners.Find(ctx, job.RunnerName)
	if err != nil {
		log.WithError(err).Warnf("hook: runner '%s' not found in datastore", job.RunnerName)
		return nil // Non-fatal: log and continue
	}

	runner.Busy = false
	runner.Status = core.RunnerStatusCompleted
	runner.Completed = time.Now().Unix()
	runner.Updated = time.Now().Unix()

	if err := h.runners.Update(ctx, runner); err != nil {
		return fmt.Errorf("failed to update runner '%s': %w", job.RunnerName, err)
	}

	log.Infof("hook: synced runner '%s' to completed", job.RunnerName)
	return nil
}

// resolveJobSpecification finds a matching label for the job and populates
// the job's OS, Arch, Memory, and VCPU fields from the label specification.
func (h *handler) resolveJobSpecification(job *core.Job, log *logrus.Entry) error {
	// Find the first matching label from the job's requested labels
	var matchedLabel core.Label
	for _, requestedLabel := range job.Labels {
		if label, ok := h.labels[requestedLabel]; ok {
			matchedLabel = label
			break
		}
	}

	// Populate job specification from resolved label
	job.OS = matchedLabel.OS
	job.Arch = matchedLabel.Arch
	job.Memory = matchedLabel.Memory
	job.VCPU = matchedLabel.VCPU

	log.Debugf("hook: resolved job specification - label: %s, os: %s, arch: %s, memory: %dMB, vcpu: %d",
		matchedLabel.ID, job.OS, job.Arch, job.Memory, job.VCPU)

	return nil
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
