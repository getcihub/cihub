package manager

import (
	"context"
	"fmt"
	"time"

	"github.com/dchest/uniuri"
	"github.com/sirupsen/logrus"

	"github.com/getcihub/cihub/core"
	"github.com/getcihub/cihub/store/shared/db"
)

// RunnerManager encapsulates complex runner operations and provides
// a simplified interface for runner agents. It manages the lifecycle of
// self-hosted GitHub Actions runners, including job assignment, runner
// registration, and cancellation monitoring.
type RunnerManager interface {
	// Request requests the next available job from the queue that matches
	// the agent's labels. It returns the job if found, or nil if no matching
	// jobs are available.
	Request(ctx context.Context, labels []string) (*core.Job, error)

	// Accept accepts a job for execution. This operation uses optimistic
	// locking to prevent multiple agents from executing the same job.
	//
	// It is possible for multiple agents to pull the same job from the queue.
	// The system uses optimistic locking at the database-level to prevent
	// multiple agents from executing the same job. If the job has already
	// been accepted by another agent, this method returns an error.
	Accept(ctx context.Context, jobID int64, machine string) (*core.Runner, error)

	// Watch watches the runner for cancellation.
	// It returns true if the runner has been cancelled, false otherwise.
	// The agent should call this method periodically during job execution to
	// check for cancellation requests.
	Watch(ctx context.Context, runnerID int64) (bool, error)
}

// New returns a new RunnerManager.
func New(
	jobs core.JobStore,
	runners core.RunnerStore,
	runnerz core.RunnerService,
	scheduler core.Scheduler,
	users core.UserStore,
) RunnerManager {
	return &Manager{
		Jobs:      jobs,
		Runners:   runners,
		Runnerz:   runnerz,
		Scheduler: scheduler,
		Users:     users,
	}
}

// Manager provides a simplified interface to the runner agent so that it
// can more easily interact with the server.
type Manager struct {
	Jobs      core.JobStore
	Runners   core.RunnerStore
	Runnerz   core.RunnerService
	Scheduler core.Scheduler
	Users     core.UserStore
}

// Request requests the next available job from the queue that matches
// the agent's labels.
func (m *Manager) Request(ctx context.Context, labels []string) (*core.Job, error) {
	logger := logrus.WithField("labels", labels)
	logger.Debugln("manager: request queued job")

	job, err := m.Scheduler.Request(ctx, labels)
	if err != nil && ctx.Err() != nil {
		logger.Debugln("manager: context canceled")
		return nil, err
	}

	if err != nil {
		logger = logger.WithError(err)
		logger.Warnln("manager: request queued job error")
		return nil, err
	}

	return job, nil
}

// Accept accepts a job for execution with optimistic locking.
func (m *Manager) Accept(ctx context.Context, jobID int64, machine string) (*core.Runner, error) {
	logger := logrus.WithFields(
		logrus.Fields{
			"job.id":  jobID,
			"machine": machine,
		},
	)
	logger.Debugln("manager: accept job")

	job, err := m.Jobs.Find(ctx, jobID)
	if err != nil {
		logger = logger.WithError(err)
		logger.Warnln("manager: cannot find job")
		return nil, err
	}

	now := time.Now()

	job.Accepted = now.Unix()
	job.Machine = machine
	job.Updated = now.Unix()

	// Step 1.
	// Accept job with optimistic locking

	err = m.Jobs.Update(ctx, job)
	if err == db.ErrOptimisticLock {
		logger = logger.WithError(err)
		logger.Debugln("manager: job processed by another agent")
		return nil, err
	} else if err != nil {
		logger = logger.WithError(err)
		logger.Debugln("manager: cannot update job")
		return nil, err
	}

	logger.Debugln("manager: job accepted")

	// Step 2.
	// Register a runner on GitHub

	runner, err := m.Runnerz.Create(ctx, core.CreateRunnerOpts{
		InstallationID: job.InstallationID,
		Name:           fmt.Sprintf("cihub-%s", uniuri.NewLen(8)),
		Owner:          job.Owner,
		Repo:           job.Repo,
		Labels:         job.Labels,
		GroupID:        1,
	})

	if err != nil {
		logger.WithError(err).
			Errorln("manager: failed to create runner on GitHub, rollback")

		// Rollback, marking job as available again
		job.Accepted = 0
		job.Machine = ""
		if err := m.Jobs.Update(ctx, job); err != nil {
			logger.WithError(err).
				Errorln("manager: failed to rollback job")
		}

		return nil, fmt.Errorf("manager: failed to create GitHub runner, err: %w", err)
	}

	// Step 3.
	// Save runner to datastore

	runner.Status = core.RunnerStatusCreating
	err = m.Runners.Create(ctx, runner)
	if err != nil {
		logger.WithError(err).
			Errorln("manager: failed to save runner to datastore")
		return nil, err
	}

	return runner, nil
}

// Watch watches the runner for cancellation.
func (m *Manager) Watch(ctx context.Context, runnerID int64) (bool, error) {
	// TODO: Implement watch logic
	// 1. Find runner by GitHub runner ID
	// 2. Check if runner.Cancelled is true
	// 3. Find associated job
	// 4. Check if job.Status is completed or job.Conclusion is cancelled
	// 5. Optionally check GitHub API for workflow run cancellation
	// 6. Return true if cancelled/completed, false otherwise
	return false, nil
}
