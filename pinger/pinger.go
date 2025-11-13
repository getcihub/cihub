package pinger

import (
	"context"
	"errors"
	"time"

	"github.com/getcihub/cihub/client"
	"github.com/getcihub/cihub/core"
	"github.com/getcihub/cihub/logger"
)

type Pinger struct {
	client    core.Client
	resourcez core.ResourceService
}

func New(client core.Client, resourcez core.ResourceService) *Pinger {
	return &Pinger{
		client:    client,
		resourcez: resourcez,
	}
}

func (p *Pinger) Start(ctx context.Context, interval time.Duration) error {
	ticker := time.NewTicker(interval)
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
			err := p.ping(ctx)

			// Exist if machine not registered
			if errors.Is(err, client.ErrMachineNotFound) {
				return nil
			}
		}
	}
}

// ping gather machine resources and ping remote server.
func (p *Pinger) ping(ctx context.Context) error {
	resources, err := p.resourcez.Report(ctx)
	if err != nil {
		logger.FromContext(ctx).
			WithError(err).
			Warnln("pinger: failed to gather machine resources")
		return err
	}

	err = p.client.Ping(ctx, resources)
	if errors.Is(err, client.ErrMachineNotFound) {
		logger.FromContext(ctx).
			WithError(err).
			Infoln("pinger: machine not registered on server")
		return err
	}

	return err
}
