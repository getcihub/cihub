package core

import "context"

// Installation represents a GitHub app installation
type Installation struct {
	Avatar string `json:"avatar"`
	ID     int64  `json:"id"`
	Name   string `json:"name"`
}

type InstallationService interface {
	// List returns a list of installation to which the
	// user is has access to.
	List(context.Context, *User) ([]*Installation, error)

	// Membership returns true if the user is a member
	// of the installation, and true if the user is an
	// of the installation.
	Membership(context.Context, *User, string) (bool, bool, error)
}
