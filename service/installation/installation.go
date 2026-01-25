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

	out, _, err := client.Apps.ListUserInstallations(ctx, &github.ListOptions{PerPage: 100})
	if err != nil {
		return nil, err
	}

	var installations []*core.Installation
	for _, installation := range out {
		installations = append(installations, &core.Installation{
			Avatar: installation.Account.GetAvatarURL(),
			ID:     installation.GetID(),
			Login:  installation.Account.GetLogin(),
		})
	}

	return installations, nil
}

func (s *service) Membership(ctx context.Context, user *core.User, org string) (bool, bool, error) {
	if user.Login == org {
		return true, true, nil
	}

	err := s.refresh.Refresh(ctx, user, false)
	if err != nil {
		return false, false, err
	}

	client, err := s.client.NewTokenClient(user.Access)
	if err != nil {
		return false, false, err
	}

	result, _, err := client.Organizations.GetOrgMembership(ctx, "", org)
	if err != nil {
		return false, false, err
	}

	switch {
	case result.GetState() != "active":
		return false, false, nil
	case result.GetRole() == "admin":
		return true, true, nil
	case result.GetRole() == "member":
		return true, false, nil
	default:
		return false, false, nil
	}
}
