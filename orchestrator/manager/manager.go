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

var noContext = context.Background()

// RunnerManager encapsulates complex runner operations and provides
// a simplified interface for runner agents. It manages the lifecycle of
// self-hosted GitHub Actions runners, including job assignment, runner
// registration, and cancellation monitoring.
type RunnerManager interface {
	// Request requests the next available job from the queue that matches
	// the agent's capacities. It returns the job if found, or nil if no matching
	// jobs are available.
	Request(ctx context.Context, params *core.Filter) (*core.Job, error)

	// Accept accepts a job for execution. This operation uses optimistic
	// locking to prevent multiple agents from executing the same job.
	//
	// It is possible for multiple agents to pull the same job from the queue.
	// The system uses optimistic locking at the database-level to prevent
	// multiple agents from executing the same job. If the job has already
	// been accepted by another agent, this method returns an error.
	Accept(ctx context.Context, jobID int64, machine string) error

	// Register registers a runner for the job on GitHub and retrieves its
	// just-in-time configuration. The runner acts as the compute environment
	// where the job will execute.
	Register(ctx context.Context, jobID int64) (*core.RunnerWithToken, error)

	// Started notifies the manager that the runner is starting execution.
	// This should be called immediately before starting the Firecracker VM.
	Started(ctx context.Context, runnerID int64) error

	// Completed notifies the manager that the runner has finished execution.
	// This should be called after the VM exits and cleanup begins.
	// It accepts a status string to indicate success/error outcomes.
	Completed(ctx context.Context, runnerID int64, status string) error

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
func (m *Manager) Request(ctx context.Context, params *core.Filter) (*core.Job, error) {
	logger := logrus.WithFields(
		logrus.Fields{
			"arch":   params.Arch,
			"owner":  params.Owner,
			"memory": params.Memory,
			"vcpu":   params.VCPU,
		},
	)
	logger.Debugln("manager: request queued job")

	job, err := m.Scheduler.Request(ctx, params)
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
func (m *Manager) Accept(ctx context.Context, id int64, machine string) error {
	logger := logrus.WithFields(
		logrus.Fields{
			"machine": machine,
			"job.id":  id,
		},
	)
	logger.Debugln("manager: accept job")

	job, err := m.Jobs.Find(noContext, id)
	if err != nil {
		logger = logger.WithError(err)
		logger.Warnln("manager: cannot find job")
		return err
	}

	if job.Machine != "" {
		logger.Debugln("manager: job already assigned. abort.")
		return db.ErrOptimisticLock
	}

	now := time.Now()

	job.Machine = machine
	job.Accepted = now.Unix()
	job.Updated = now.Unix()

	err = m.Jobs.Update(noContext, job)
	if err == db.ErrOptimisticLock {
		logger = logger.WithError(err)
		logger.Debugln("manager: job processed by another agent")
	} else if err != nil {
		logger = logger.WithError(err)
		logger.Debugln("manager: cannot update job")
	} else {
		logger.Debugln("manager: job accepted")
	}

	return err
}

func (m *Manager) Register(ctx context.Context, id int64) (*core.RunnerWithToken, error) {
	logger := logrus.WithField("job-id", id)
	logger.Debugln("manager: fetching job details")

	job, err := m.Jobs.Find(noContext, id)
	if err != nil {
		logger = logger.WithError(err)
		logger.Warnln("manager: cannot fin job")
		return nil, err
	}

	logger.Debugln("manager: registering runner")
	runner, err := m.Runnerz.Create(noContext, core.CreateRunnerOpts{
		InstallationID: job.InstallationID,
		Name:           fmt.Sprintf("cihub-%s", uniuri.NewLen(8)),
		Owner:          job.Owner,
		Labels:         job.Labels,
		GroupID:        1,
	})

	if err != nil {
		logger = logger.WithError(err)
		logger.Warnln("manager: cannot register runner")
		return nil, err
	}

	err = m.Runners.Create(ctx, runner)
	if err != nil {
		logger = logger.WithError(err)
		logger.Warnln("manager: cannot create runner")
		return nil, err
	}

	return &core.RunnerWithToken{
		Runner: runner,
		Token:  runner.Token,
	}, nil
}

// Started notifies the manager that the runner is starting execution.
// It updates the runner's started timestamp and marks it as idle. Status
// will be modified when receiving job.
func (m *Manager) Started(ctx context.Context, runnerID int64) error {
	logger := logrus.WithField("runner-id", runnerID)
	logger.Debugln("manager: marking runner as started")

	runner, err := m.Runners.FindID(ctx, runnerID)
	if err != nil {
		logger = logger.WithError(err)
		logger.Warnln("manager: cannot find runner")
		return err
	}

	now := time.Now()
	runner.Started = now.Unix()
	runner.Status = core.RunnerStatusIdle
	runner.Updated = now.Unix()

	err = m.Runners.Update(noContext, runner)
	if err != nil {
		logger = logger.WithError(err)
		logger.Warnln("manager: cannot update runner")
		return err
	}

	logger.Debugln("manager: runner marked as started")
	return nil
}

// Completed notifies the manager that the runner has finished execution.
// It updates the runner's stopped and completed timestamps, and sets its status.
func (m *Manager) Completed(ctx context.Context, runnerID int64, status string) error {
	logger := logrus.WithFields(
		logrus.Fields{
			"runner-id": runnerID,
			"status":    status,
		},
	)
	logger.Debugln("manager: marking runner as completed")

	runner, err := m.Runners.FindID(noContext, runnerID)
	if err != nil {
		logger = logger.WithError(err)
		logger.Warnln("manager: cannot find runner")
		return err
	}

	now := time.Now()
	runner.Completed = now.Unix()
	runner.Stopped = now.Unix()
	runner.Status = status
	runner.Busy = false
	runner.Updated = now.Unix()

	err = m.Runners.Update(noContext, runner)
	if err != nil {
		logger = logger.WithError(err)
		logger.Warnln("manager: cannot update runner")
		return err
	}

	logger.Debugln("manager: runner marked as completed")
	return nil
}

// Watch watches the runner for cancellation.
func (m *Manager) Watch(ctx context.Context, id int64) (bool, error) {
	return m.Scheduler.Cancelled(ctx, id)
}
