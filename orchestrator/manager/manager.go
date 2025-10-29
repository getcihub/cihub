package manager

import (
	"context"
	"time"

	"github.com/sirupsen/logrus"

	"github.com/getcihub/cihub/core"
	"github.com/getcihub/cihub/store/shared/db"
)

var noContext = context.Background()

// New returns a new RunnerManager.
func New(runners core.RunnerStore, runnerz core.RunnerService, scheduler core.Scheduler) core.RunnerManager {
	return &manager{
		runners:   runners,
		runnerz:   runnerz,
		scheduler: scheduler,
	}
}

// Manager provides a simplified interface to the runner agent so that it
// can more easily interact with the server.
type manager struct {
	runners   core.RunnerStore
	runnerz   core.RunnerService
	scheduler core.Scheduler
}

// Request requests the next available runner that matches
// machine's capacities. Returns the runner if found, nil if no matching
// runner available.
func (m *manager) Request(ctx context.Context, params *core.Filter) (*core.Runner, error) {
	logger := logrus.WithFields(
		logrus.Fields{
			"arch":  params.Arch,
			"cpu":   params.CPU,
			"owner": params.Owner,
			"ram":   params.RAM,
		},
	)
	logger.Debugln("manager: request pending runner")

	runner, err := m.scheduler.Request(ctx, params)
	if err != nil && ctx.Err() != nil {
		logger = logger.WithError(err)
		logger.Traceln("manager: context canceled")
		return nil, err
	}

	if err != nil {
		logger = logger.WithError(err)
		logger.Warnln("manager: cannot request pending runner")
		return nil, err
	}

	return runner, nil
}

// Accept accepts a runner for execution. This operation uses optimistic
// locking to prevent multiple agents from executing the same runner.
func (m *manager) Accept(ctx context.Context, name, machine string) error {
	logger := logrus.WithFields(
		logrus.Fields{
			"machine": machine,
			"runner":  name,
		},
	)
	logger.Debugln("manager: accept runner")

	runner, err := m.runners.Find(ctx, name)
	if err != nil {
		logger = logger.WithError(err)
		logger.Warnf("manager: cannot find runner %s", name)
		return err
	}

	if runner.Machine != "" {
		logger.
			WithField("machine", runner.Machine).
			Debugln("manager: runner already assigned. abort.")
		return db.ErrOptimisticLock
	}

	now := time.Now()

	runner.Accepted = now.Unix()
	runner.Machine = machine
	runner.Updated = now.Unix()

	err = m.runners.Update(noContext, runner)
	if err == db.ErrOptimisticLock {
		logger = logger.WithError(err)
		logger.Debugln("manager: runner processed by another agent")
	} else if err != nil {
		logger = logger.WithError(err)
		logger.Debugln("manager: cannot update runner")
	} else {
		logger.Debugln("manager: runner accepted")
	}

	return err
}

func (m *manager) Register(ctx context.Context, name string) (*core.RunnerWithToken, error) {
	logger := logrus.WithField("runner_name", name)

	runner, err := m.runners.Find(noContext, name)
	if err != nil {
		logger = logger.WithError(err)
		logger.Warnln("manager: cannot fin job")
		return nil, err
	}

	// Runner already registered
	if runner.ID != 0 && runner.Token != "" {
		return &core.RunnerWithToken{
			Runner: runner,
			Token:  runner.Token,
		}, nil
	}

	logger.Debugln("manager: registering runner")
	r, err := m.runnerz.Register(noContext, core.RegisterRunnerOpts{
		InstallationID: runner.InstallationID,
		Name:           runner.Name,
		Owner:          runner.Owner,
		Labels:         runner.Labels,
		GroupID:        1,
	})

	if err != nil {
		logger = logger.WithError(err)
		logger.Warnln("manager: cannot register runner")
		return nil, err
	}

	runner.ID = r.ID
	runner.Token = r.Token
	runner.Updated = time.Now().Unix()

	err = m.runners.Update(ctx, runner)
	if err != nil {
		logger = logger.WithError(err)
		logger.Warnln("manager: cannot update runner")
		return nil, err
	}

	return &core.RunnerWithToken{
		Runner: runner,
		Token:  runner.Token,
	}, nil
}

// Watch watches the runner for cancellation.
func (m *manager) Watch(ctx context.Context, name string) (bool, error) {
	return m.scheduler.Cancelled(ctx, name)
}
