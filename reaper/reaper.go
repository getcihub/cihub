package reaper

import (
	"context"
	"runtime/debug"
	"time"

	"github.com/hashicorp/go-multierror"
	"github.com/sirupsen/logrus"

	"github.com/getcihub/cihub/core"
)

// The reaper monitors and terminates zombie runners that remain stuck in a
// pending state for an excessive duration. This prevents resource leaks and
// ensures the system can recover from failed runner initialization attempts.
type Reaper struct {
	Reclaim   time.Duration
	Runners   core.RunnerStore
	Runnerz   core.RunnerService
	Scheduler core.Scheduler
}

// New returns a new Reaper.
func New(
	runners core.RunnerStore,
	runnerz core.RunnerService,
	scheduler core.Scheduler,
	reclaim time.Duration) *Reaper {
	if reclaim == 0 {
		reclaim = time.Minute
	}

	return &Reaper{
		Reclaim:   reclaim,
		Runners:   runners,
		Runnerz:   runnerz,
		Scheduler: scheduler,
	}
}

// Start starts the reaper.
func (r *Reaper) Start(ctx context.Context, dur time.Duration) error {
	ticker := time.NewTicker(dur)
	defer ticker.Stop()

	for {
		select {
		case <-ctx.Done():
			return nil
		case <-ticker.C:
			// This error is ignored on purpose. The system
			// should not exit the runner on error. The reap
			// function logs all errors, which should be enough
			// to surface potential issues to an administrator.
			r.reap(ctx)
		}
	}
}

func (r *Reaper) reap(ctx context.Context) error {
	defer func() {
		// taking the paranoid approach to recover from
		// a panic that should absolutely never happen.
		if r := recover(); r != nil {
			logrus.Errorf("reaper: unexpected panic: %s", r)
			debug.PrintStack()
		}
	}()

	logrus.Traceln("reaper: find zombie runners")

	runners, err := r.Runners.ListStatus(ctx, core.RunnerStatusPending)
	if err != nil {
		logrus.WithError(err).
			Errorln("reaper: cannot get pending runners")
		return err
	}

	var result error
	for _, runner := range runners {
		logger := logrus.WithFields(
			logrus.Fields{
				"runner.name":    runner.Name,
				"runner.id":      runner.ID,
				"runner.created": runner.Created,
				"runner.owner":   runner.Owner,
				"runner.status":  runner.Status,
			},
		)

		// If a runner is pending for longer than the maximum
		// reclaim time, the runner is maybe cancelled.
		if isExceeded(runner.Created, r.Reclaim) {
			logger.Debugln("reaper: runner reclaime timeout exceeded, checking cancellation...")

			err = r.reapMaybe(ctx, runner)
			if err != nil {
				logger.WithError(err).
					Errorln("reaper: cannot cancel runner")
				result = multierror.Append(result, err)
			}
		} else {
			logger.Traceln("reaper: ignore runner, reclaim timeout not exceeded")
		}
	}

	return nil
}

func (r *Reaper) reapMaybe(ctx context.Context, runner *core.Runner) error {
	logger := logrus.WithFields(
		logrus.Fields{
			"runner.name":    runner.Name,
			"runner.id":      runner.ID,
			"runner.created": runner.Created,
			"runner.owner":   runner.Owner,
			"runner.status":  runner.Status,
		},
	)

	// if the runner ID is not set (equals 0), it indicates
	// the runner was never registered with GitHub, but still has
	// an entry in the database. in this case, we can simply
	// delete the datbase entry.
	if runner.ID == 0 {
		logger.Infoln("reaper: runner never registered, nothing to unregister")

		err := r.Runners.Delete(ctx, runner)
		if err != nil {
			logger = logger.WithError(err)
			logger.Warnln("reaper: failed to delete runner from datastore")
			return err
		}

		return nil
	}

	logger.Debugln("reaper: get runner status from GitHub")

	s, err := r.Runnerz.Find(ctx, runner.Owner, runner.InstallationID, runner.ID)
	if err != nil {
		logger = logger.WithError(err)
		logger.Warnln("reaper: failed to get runner from GitHub")
		return err
	}

	// Do not cancel the runner if is busy working
	if s.Busy {
		logger.Infoln("reaper: runner is busy, skipping cancellation")
		return nil
	}

	// Unregister the runner from GitHub.
	logger.Infoln("reaper: unregistering runner from GitHub")
	err = r.Runnerz.Delete(ctx, runner)
	if err != nil {
		logger = logger.WithError(err)
		logger.Warnln("reaper: failed to unregister runner from GitHub")
		return err
	}

	// Update runner from datastore after unregistered from GitHub
	//
	// Note: If GitHub runner is deleted but DB update failed
	// This is safer than the opposite - the runner will be reaped on next cycle
	runner.Busy = false
	runner.Cancelled = true
	runner.Status = core.RunnerStatusCompleted
	runner.Stopped = time.Now().Unix()
	if runner.Started == 0 {
		runner.Started = time.Now().Unix()
	}

	err = r.Runners.Update(ctx, runner)
	if err != nil {
		logger.WithError(err).
			Warnln("reaper: cannot update runner status to cancelled")
		return err
	}

	// notify the scheduler to cancel the runner. this will
	// instruct agents subscribing to the scheduler to
	// cancel execution.
	err = r.Scheduler.Cancel(ctx, runner.ID)
	if err != nil {
		logger.WithError(err).
			Warnln("reaper: cannot signal cancelled runner is complete")
	}

	logger.Infoln("reaper: successfully reaped runner")

	return nil
}
