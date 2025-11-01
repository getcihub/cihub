package job

import (
	"context"
	"encoding/json"
	"fmt"
	"time"

	"github.com/dchest/uniuri"
	"github.com/google/go-github/v76/github"
	"github.com/palantir/go-githubapp/githubapp"
	"github.com/pkg/errors"
	"github.com/sirupsen/logrus"

	"github.com/getcihub/cihub/core"
	"github.com/getcihub/cihub/label"
	"github.com/getcihub/cihub/logger"
)

type handler struct {
	jobs      core.JobStore
	runners   core.RunnerStore
	scheduler core.Scheduler
}

func New(
	jobs core.JobStore,
	runners core.RunnerStore,
	scheduler core.Scheduler,
) githubapp.EventHandler {
	return &handler{
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
			"action":      action,
			"delivery_id": deliveryID,
			"event":       eventType,
			"job_id":      job.ID,
			"labels":      job.Labels,
			"owner":       job.Owner,
			"repo":        job.Repo,
			"run_id":      job.RunID,
			"runner_id":   job.RunnerID,
			"runner_name": job.RunnerName,
			"status":      job.Status,
		},
	)
	log.Infoln("hook: received workflow job event")

	// Check if job has a CIHub label
	if !label.Has(job.Labels) {
		log.Debugln("hook: no cihub label found, ignore event")
		return nil
	}

	// Parse the CIHub label to get resource specifications
	lbl, err := label.Resolve(job.Labels)
	if err != nil {
		log.WithError(err).
			Warnln("hook: invalid cihub label, skipping event")
		return nil
	}

	// Validate the parsed label
	if err := lbl.Validate(); err != nil {
		log.WithError(err).
			Warnln("hook: invalid label specification, skipping event")
		return nil
	}

	// Route to action-specific handler
	switch action {
	case "waiting":
		return h.handleWaiting(ctx, log, job)
	case "queued":
		return h.handleQueued(ctx, log, job, lbl)
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

func (h *handler) handleQueued(ctx context.Context, log *logrus.Entry, job *core.Job, lbl *label.Label) error {
	// Try to find existing job
	existing, err := h.jobs.Find(ctx, job.ID)
	if err != nil {
		// Create new job record
		now := time.Now().Unix()
		job.Created = now
		job.Updated = now
		job.Status = core.JobStatusQueued

		if err := h.jobs.Create(ctx, job); err != nil {
			return fmt.Errorf("hook: failed to create queued job, err: %w", err)
		}

		log.Infoln("hook: created queued job")
	} else {
		// Update existing job
		job.Version = existing.Version
		job.Created = existing.Created
		job.Updated = time.Now().Unix()
		job.Status = core.JobStatusQueued

		if err := h.jobs.Update(ctx, job); err != nil {
			return fmt.Errorf("hook: failed to update queued job, err: %w", err)
		}

		log.Infoln("hook: updated queued job")
	}

	// Create and schedule runner for this job
	runner := h.createRunnerFromJob(job, lbl)
	if err := h.runners.Create(ctx, runner); err != nil {
		log = log.WithError(err)
		log.Warnln("hook: cannot create runner")
		return err
	}

	return h.scheduler.Schedule(ctx, runner)
}

func (h *handler) handleInProgress(ctx context.Context, log *logrus.Entry, job *core.Job) error {
	// Try to find existing job
	existing, err := h.jobs.Find(ctx, job.ID)
	if err != nil {
		// Create new job record
		now := time.Now().Unix()
		job.Created = now
		job.Updated = now
		job.Status = core.JobStatusInProgress

		if err := h.jobs.Create(ctx, job); err != nil {
			return fmt.Errorf("hook: failed to create in_progress job, err: %w", err)
		}

		log.Infoln("hook: created in_progress job")
	} else {
		// Update existing job
		job.Version = existing.Version
		job.Created = existing.Created
		job.Updated = time.Now().Unix()
		job.Status = core.JobStatusInProgress

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
		job.Status = core.JobStatusCompleted

		if err := h.jobs.Create(ctx, job); err != nil {
			return fmt.Errorf("hook: failed to create completed job, err: %w", err)
		}

		log.Infoln("hook: created completed job")
	} else {
		// Update existing job
		job.Version = existing.Version
		job.Created = existing.Created
		job.Updated = time.Now().Unix()
		job.Status = core.JobStatusCompleted

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

	runner.Status = core.RunnerStatusBusy
	runner.Started = time.Now().Unix()
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

	runner.Status = core.RunnerStatusCompleted
	runner.Stopped = time.Now().Unix()
	runner.Updated = time.Now().Unix()

	if err := h.runners.Update(ctx, runner); err != nil {
		return fmt.Errorf("failed to update runner '%s': %w", job.RunnerName, err)
	}

	log.Infof("hook: synced runner '%s' to completed", job.RunnerName)
	return nil
}

// createRunnerFromJob creates a core.Runner from a core.Job for scheduling.
// The runner is created in pending state and will be assigned to a machine when available.
// The runner specification is populated from the provided label.
func (h *handler) createRunnerFromJob(job *core.Job, lbl *label.Label) *core.Runner {
	return &core.Runner{
		InstallationID: job.InstallationID,
		Name:           fmt.Sprintf("cihub-%s", uniuri.NewLen(16)),
		Owner:          job.Owner,
		Status:         core.RunnerStatusPending,
		Arch:           lbl.Arch,
		CPU:            lbl.CPU,
		RAM:            lbl.RAM,
		Labels:         job.Labels,
		Created:        time.Now().Unix(),
		Updated:        time.Now().Unix(),
	}
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
		URL:            wj.GetHTMLURL(),
	}

	// Extract author information from the event actor if available
	if event.Sender != nil {
		job.AuthorLogin = event.Sender.GetLogin()
		job.AuthorAvatar = event.Sender.GetAvatarURL()
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
