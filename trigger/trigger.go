package trigger

import (
	"context"
	"fmt"
	"runtime/debug"
	"time"

	"github.com/dchest/uniuri"
	"github.com/getcihub/cihub/core"
	"github.com/getcihub/cihub/logger"
)

const (
	labelPattern = `^cihub-(\d+)cpu-(\d+)(mb|gb)(?:-(amd64|arm64))?$`
	labelPrefix  = "cihub-"
)

type triggerer struct {
	runners   core.RunnerStore
	runnerz   core.RunnerService
	scheduler core.Scheduler
}

// New returns a new runner triggerer.
func New(
	runners core.RunnerStore,
	runnerz core.RunnerService,
	scheduler core.Scheduler,
) core.Triggerer {
	return &triggerer{
		runners:   runners,
		runnerz:   runnerz,
		scheduler: scheduler,
	}
}

func (t *triggerer) Trigger(ctx context.Context, hook *core.Hook) (*core.Runner, error) {
	log := logger.FromContext(ctx)

	log.Debugln("trigger: received")
	defer func() {
		// taking the paranoid approach to recover from
		// a panic that should absolutely never happen.
		if r := recover(); r != nil {
			log.Errorf("trigger: unexpected panic: %s", r)
			debug.PrintStack()
		}
	}()

	label, ok := extractLabel(hook.Labels)
	if !ok {
		return nil, nil
	}

	resource, err := extractResource(label)
	if err != nil {
		return nil, err
	}

	runner := &core.Runner{
		Arch:           resource.Arch,
		CPU:            resource.CPU,
		Created:        time.Now().Unix(),
		InstallationID: hook.InstallationID,
		Labels:         hook.Labels,
		Name:           fmt.Sprintf("cihub-%s", uniuri.NewLen(16)),
		Owner:          hook.Owner,
		RAM:            resource.RAMTotal,
		GroupID:        hook.RunnerGroupID,
		Status:         core.RunnerStatusPending,
		Updated:        time.Now().Unix(),
	}

	// register runner to github
	opts := core.RegisterRunnerOpts{
		GroupID:        runner.GroupID,
		InstallationID: runner.InstallationID,
		Labels:         runner.Labels,
		Name:           runner.Name,
		Owner:          runner.Owner,
	}

	jit, err := t.runnerz.Register(ctx, opts)
	if err != nil {
		log := log.WithError(err)
		log.Warnln("trigger: cannot register runner")
		return nil, err
	}

	runner.ID = jit.ID
	runner.Status = core.RunnerStatusRegistered
	runner.Token = jit.Token

	err = t.runners.Create(ctx, runner)
	if err != nil {
		log := log.WithError(err)
		log.Warnln("trigger: cannot create runner")
		return nil, err
	}

	err = t.scheduler.Schedule(ctx, runner)
	if err != nil {
		log := log.WithError(err)
		log.Warnln("trigger: cannot enqueue runner")
		return nil, err
	}

	return runner, nil
}
