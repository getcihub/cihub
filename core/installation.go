package core

import "context"

const (
	InstallationTypeOrganization = "organization"
	InstallationTypeUser         = "user"
)

type (
	// Installation represents a GitHub app installation
	Installation struct {
		Avatar string `json:"avatar_url"`
		ID     int64  `json:"id"`
		Login  string `json:"login"`
	}

	// InstallationService provides access to installation from GitHub.
	InstallationService interface {
		// List returns a list of organization to which the
		// user is a member.
		List(context.Context, *User) ([]*Installation, error)

		// Membership returns true if the user is a member
		// of the organization, and true if the user is an
		// of the organization.
		Membership(context.Context, *User, string) (bool, bool, error)
	}
)
