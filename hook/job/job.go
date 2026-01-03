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

	"github.com/getcihub/cihub/core"
	"github.com/getcihub/cihub/label"
	"github.com/getcihub/cihub/logger"
)

type handler struct {
	runners   core.RunnerStore
	scheduler core.Scheduler
}

func New(runners core.RunnerStore, scheduler core.Scheduler) githubapp.EventHandler {
	return &handler{runners, scheduler}
}

func (h *handler) Handles() []string { return []string{"workflow_job"} }

func (h *handler) Handle(ctx context.Context, eventType, deliveryID string, payload []byte) error {
	// Parse the GitHub webhook payload
	var event github.WorkflowJobEvent
	if err := json.Unmarshal(payload, &event); err != nil {
		return errors.Wrap(err, "failed to parse workflow job event payload")
	}

	// Sanity check
	switch {
	case event.WorkflowJob == nil:
		return errors.New("missing workflow_job in event payload")
	case event.Repo == nil:
		return errors.New("missing repository in event payload")
	case event.Installation == nil:
		return errors.New("missing installation in event payload")
	}

	// Extract labels
	labels := event.GetWorkflowJob().Labels
	if len(labels) == 0 || !label.Has(labels) {
		logger.FromContext(ctx).
			WithField("labels", labels).
			Debugln("hook: skipping hook. no cihub labels found")
		return nil
	}

	// Parse the CIHub label to get resource specifications
	lbl, err := label.Resolve(labels)
	if err != nil {
		logger.FromContext(ctx).
			WithError(err).
			WithField("labels", labels).
			Warnln("hook: skipping hook. invalid cihub label")
		return nil
	}

	// Get runner's name from event or generate
	name := event.GetWorkflowJob().GetRunnerName()
	if name == "" {
		name = fmt.Sprintf("cihub-%s", uniuri.NewLen(16))
	}

	runner := &core.Runner{
		InstallationID: event.GetInstallation().GetID(),
		Arch:           lbl.Arch,
		CPU:            lbl.CPU,
		Created:        time.Now().Unix(),
		Labels:         labels,
		Name:           name,
		Owner:          event.GetRepo().GetOwner().GetLogin(),
		RAM:            lbl.RAM,
		Status:         core.RunnerStatusPending,
		Updated:        time.Now().Unix(),
	}

	// Route to action-specific handler
	switch event.GetAction() {
	case "queued":
		return h.handleQueued(ctx, runner)
	case "in_progress":
		return h.handleInProgress(ctx, runner)
	case "completed":
		return h.handleCompleted(ctx, runner)
	default:
		return nil
	}
}

func (h *handler) handleQueued(ctx context.Context, runner *core.Runner) error {
	err := h.runners.Create(ctx, runner)
	if err != nil {
		logger.FromContext(ctx).
			WithError(err).
			Warnln("hook: cannot create runner")
		return err
	}

	return h.scheduler.Schedule(ctx, runner)
}

func (h *handler) handleInProgress(ctx context.Context, runner *core.Runner) error {
	runner.Started = time.Now().Unix()
	runner.Status = core.RunnerStatusBusy
	runner.Updated = time.Now().Unix()

	err := h.runners.Update(ctx, runner)
	if err != nil {
		logger.FromContext(ctx).
			WithError(err).
			Warnln("hook: cannot mark runner in progress")
		return err
	}

	return nil
}

func (h *handler) handleCompleted(ctx context.Context, runner *core.Runner) error {
	runner.Stopped = time.Now().Unix()
	runner.Status = core.RunnerStatusCompleted
	runner.Updated = time.Now().Unix()

	err := h.runners.Update(ctx, runner)
	if err != nil {
		logger.FromContext(ctx).
			WithError(err).
			Warnln("hook: cannot mark runner completed")
		return err
	}

	return nil
}
