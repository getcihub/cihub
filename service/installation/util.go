package installation

import (
	"github.com/google/go-github/v75/github"

	"github.com/getcihub/cihub/core"
)

func convertAccountType(s string) string {
	switch s {
	case "Organization":
		return core.InstallationTypeOrganization
	default:
		return core.InstallationTypeUser
	}
}

func convertInstallation(src *github.Installation) *core.Installation {
	var suspendedAt int64
	if !src.GetSuspendedAt().IsZero() {
		suspendedAt = src.GetSuspendedAt().Unix()
	}

	return &core.Installation{
		ID:        src.GetID(),
		Login:     src.Account.GetLogin(),
		Avatar:    src.Account.GetAvatarURL(),
		Type:      convertAccountType(src.Account.GetType()),
		Created:   src.GetCreatedAt().Unix(),
		Suspended: suspendedAt,
		Updated:   src.GetUpdatedAt().Unix(),
	}
}

func convertMembership(src *github.Membership) *core.Membership {
	return &core.Membership{
		Role:  src.GetRole(),
		State: src.GetState(),
	}
}
