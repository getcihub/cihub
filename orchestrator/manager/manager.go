package manager

import (
	"context"
	"time"

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
	Accept(ctx context.Context, jobID int64, machine string) (*core.Job, error)

	// Details returns the complete runner context including the registration
	// token needed to register the runner with GitHub.
	Details(ctx context.Context, jobID int64) (*Context, error)

	// Watch watches the runner for cancellation.
	// It returns true if the runner has been cancelled, false otherwise.
	// The agent should call this method periodically during job execution to
	// check for cancellation requests.
	Watch(ctx context.Context, runnerID int64) (bool, error)
}

// Context provides the runner execution context with the minimal
// information needed to start a GitHub Actions runner.
type Context struct {
	Token      string // GitHub runner registration token (JIT)
	RunnerName string // Unique runner name
	RunnerID   int64  // GitHub runner ID (after registration)
	Timeout    int64  // Runner timeout in seconds
}

// New returns a new RunnerManager.
func New(
	jobs core.JobStore,
	runners core.RunnerStore,
	scheduler core.Scheduler,
	users core.UserStore,
) RunnerManager {
	return &Manager{
		Jobs:      jobs,
		Runners:   runners,
		Scheduler: scheduler,
		Users:     users,
	}
}

// Manager provides a simplified interface to the runner agent so that it
// can more easily interact with the server.
type Manager struct {
	Jobs      core.JobStore
	Runners   core.RunnerStore
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
func (m *Manager) Accept(ctx context.Context, jobID int64, machine string) (*core.Job, error) {
	logger := logrus.WithFields(
		logrus.Fields{
			"job-id":  jobID,
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

	err = m.Jobs.Update(ctx, job)
	if err == db.ErrOptimisticLock {
		logger = logger.WithError(err)
		logger.Debugln("manager: job processed by another agent")
	} else if err != nil {
		logger = logger.WithError(err)
		logger.Debugln("manager: cannot update job")
	} else {
		logger.Debugln("manager: job accepted")
	}

	return job, err
}

// Details returns the complete runner context including GitHub registration token.
func (m *Manager) Details(ctx context.Context, jobID int64) (*Context, error) {
	// TODO: Implement details logic
	// 1. Find job by ID
	// 2. Find runner by job.RunnerName
	// 3. Call GitHub API to register runner and get JIT token
	// 4. Update runner with token and GitHub runner ID
	// 5. Return Context with token, runner name, runner ID, and timeout
	return nil, nil
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
