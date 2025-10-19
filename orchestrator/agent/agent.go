package agent

import (
	"context"
	"sync"

	"golang.org/x/sync/errgroup"

	"github.com/getcihub/cihub/core"
	"github.com/getcihub/cihub/orchestrator/manager"
)

type Agent struct {
	sync.Mutex

	Manager   manager.RunnerManager
	Images    core.ImageService
	Snapshots core.SnapshotService

	Kernel  string
	Labels  []string
	Machine string
	Memory  int64
	OS      string
	VCPU    int64
}

// Start starts N runner agent processes. Each process polls
// the server for pending runners to execute.
func (a *Agent) Start(ctx context.Context, n int) error {
	var g errgroup.Group
	for i := 0; i < n; i++ {
		g.Go(func() error {
			return a.start(ctx)
		})
	}
	return g.Wait()
}

func (a *Agent) start(ctx context.Context) error {
	for {
		select {
		case <-ctx.Done():
			return nil
		default:
			// This error is ignored on purpose. The system
			// should not exit the runner on error. The run
			// function logs all errors, which should be enough
			// to surface potential issues to an administrator.
			a.poll(ctx)
		}
	}
}

func (a *Agent) poll(_ context.Context) error {
	return nil
}
