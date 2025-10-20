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

func (s *service) Create(ctx context.Context, opts core.CreateRunnerOpts) (*core.Runner, error) {
	c, err := s.client.NewInstallationClient(opts.InstallationID)
	if err != nil {
		return nil, err
	}

	runner, _, err := c.Actions.GenerateRepoJITConfig(ctx, opts.Owner, opts.Repo, &github.GenerateJITConfigRequest{
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
		Repo:           opts.Repo,
		ID:             runner.GetRunner().GetID(),
		Status:         runner.GetRunner().GetStatus(),
		Created:        time.Now().Unix(),
		Token:          runner.GetEncodedJITConfig(),
	}, nil
}

func (s *service) Delete(ctx context.Context, runner *core.Runner) error {
	c, err := s.client.NewInstallationClient(runner.InstallationID)
	if err != nil {
		return err
	}

	_, err = c.Actions.RemoveRunner(ctx, runner.Owner, runner.Repo, runner.ID)
	if err != nil {
		return err
	}

	return nil
}
