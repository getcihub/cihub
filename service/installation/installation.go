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
	return &service{
		client:  client,
		refresh: refresh,
	}
}

func (s *service) List(ctx context.Context, user *core.User) ([]*core.Installation, error) {
	err := s.refresh.Refresh(ctx, user, false)
	if err != nil {
		return nil, err
	}

	client, err := s.client.NewTokenClient(user.Access)
	if err != nil {
		return nil, err
	}

	out, _, err := client.Apps.ListUserInstallations(ctx, &github.ListOptions{PerPage: 100})
	if err != nil {
		return nil, err
	}

	var installations []*core.Installation
	for _, install := range out {
		installations = append(installations, &core.Installation{
			Avatar: install.Account.GetAvatarURL(),
			ID:     install.GetID(),
			Name:   install.Account.GetLogin(),
		})
	}

	return installations, nil
}

func (s *service) Membership(ctx context.Context, user *core.User, name string) (bool, bool, error) {
	err := s.refresh.Refresh(ctx, user, false)
	if err != nil {
		return false, false, err
	}

	client, err := s.client.NewTokenClient(user.Access)
	if err != nil {
		return false, false, err
	}

	out, _, err := client.Organizations.GetOrgMembership(ctx, "", name)
	if err != nil {
		return false, false, err
	}

	switch {
	case out.GetState() != "active":
		return false, false, nil
	case out.GetRole() == "admin":
		return true, true, nil
	case out.GetRole() == "member":
		return true, false, nil
	default:
		return false, false, nil
	}
}
