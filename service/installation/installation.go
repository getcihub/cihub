package installation

import (
	"context"

	"github.com/google/go-github/v75/github"
	"github.com/palantir/go-githubapp/githubapp"

	"github.com/getcihub/cihub/core"
)

type service struct {
	client  githubapp.ClientCreator
	refresh core.Refresher
}

// New returns a new InstallationService.
func New(client githubapp.ClientCreator, refresh core.Refresher) core.InstallationService {
	return &service{client: client, refresh: refresh}
}

// List returns a slice of installation the user has access to.
func (s *service) List(ctx context.Context, user *core.User) ([]*core.Installation, error) {
	err := s.refresh.Refresh(ctx, user, false)
	if err != nil {
		return nil, err
	}

	client, err := s.client.NewTokenClient(user.Access)
	if err != nil {
		return nil, err
	}

	installations := []*core.Installation{}
	opts := &github.ListOptions{PerPage: 100}
	for {
		result, meta, err := client.Apps.ListUserInstallations(ctx, opts)
		if err != nil {
			return nil, err
		}
		for _, src := range result {
			installations = append(installations, convertInstallation(src))
		}
		opts.Page = meta.NextPage
		if opts.Page == 0 {
			break
		}
	}

	return installations, nil
}

// FindMembership returns the membership of the user for an organization.
func (s *service) FindMembership(ctx context.Context, user *core.User, org string) (*core.Membership, error) {
	err := s.refresh.Refresh(ctx, user, false)
	if err != nil {
		return nil, err
	}

	client, err := s.client.NewTokenClient(user.Access)
	if err != nil {
		return nil, err
	}

	result, _, err := client.Organizations.GetOrgMembership(ctx, "", org)
	if err != nil {
		return nil, err
	}

	return convertMembership(result), nil
}
