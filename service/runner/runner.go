package runner

import (
	"context"
	"time"

	"github.com/google/go-github/v75/github"
	"github.com/palantir/go-githubapp/githubapp"

	"github.com/getcihub/cihub/core"
)

type service struct {
	client githubapp.ClientCreator
}

func New(client githubapp.ClientCreator) core.RunnerService {
	return &service{client}
}

func (s *service) Register(ctx context.Context, opts core.RegisterRunnerOpts) (*core.Runner, error) {
	c, err := s.client.NewInstallationClient(opts.InstallationID)
	if err != nil {
		return nil, err
	}

	runner, _, err := c.Actions.GenerateOrgJITConfig(ctx, opts.Owner, &github.GenerateJITConfigRequest{
		Name:          opts.Name,
		RunnerGroupID: opts.GroupID,
		Labels:        opts.Labels,
	})
	if err != nil {
		return nil, err
	}

	return &core.Runner{
		Name:           opts.Name,
		InstallationID: opts.InstallationID,
		Owner:          opts.Owner,
		ID:             runner.GetRunner().GetID(),
		Status:         runner.GetRunner().GetStatus(),
		GroupID:        opts.GroupID,
		Labels:         opts.Labels,
		Created:        time.Now().Unix(),
		Token:          runner.GetEncodedJITConfig(),
	}, nil
}

func (s *service) Delete(ctx context.Context, runner *core.Runner) error {
	c, err := s.client.NewInstallationClient(runner.InstallationID)
	if err != nil {
		return err
	}

	_, err = c.Actions.RemoveOrganizationRunner(ctx, runner.Owner, runner.ID)
	if err != nil {
		return err
	}

	return nil
}

func (s *service) Find(ctx context.Context, owner string, installationID, runnerID int64) (*core.Runner, error) {
	c, err := s.client.NewInstallationClient(installationID)
	if err != nil {
		return nil, err
	}

	runner, _, err := c.Actions.GetOrganizationRunner(ctx, owner, runnerID)
	if err != nil {
		return nil, err
	}

	status := runner.GetStatus()
	if runner.GetBusy() {
		status = core.RunnerStatusBusy
	}

	return &core.Runner{
		InstallationID: installationID,
		Owner:          owner,
		ID:             runner.GetID(),
		Name:           runner.GetName(),
		Status:         status,
	}, nil
}
