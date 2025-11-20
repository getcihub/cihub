package core

import "context"

const (
	InstallationTypeOrganization = "organization"
	InstallationTypeUser         = "user"
)

type (
	// Installation represents a GitHub app installation
	Installation struct {
		ID         int64       `json:"id"`
		Login      string      `json:"login"`
		Avatar     string      `json:"avatar_url"`
		Type       string      `json:"account_type"`
		Membership *Membership `json:"membership,omitempty"`
		Created    int64       `json:"created_at"`
		Suspended  int64       `json:"suspended_at"`
		Updated    int64       `json:"updated_at"`
	}

	// InstallationService provides access to installation from GitHub.
	InstallationService interface {
		// List returns a slice of installation the user as access to.
		List(ctx context.Context, user *User) ([]*Installation, error)

		// FindMembership returns the membership of the user for an organization.
		FindMembership(ctx context.Context, user *User, org string) (*Membership, error)
	}

	// InstallationStore defines operations for working with installation on a datastore.
	InstallationStore interface {
		// Count returns a count of active installations from the datastore.
		Count(ctx context.Context) (int64, error)

		// Create persists a new installation to the datastore.
		Create(ctx context.Context, installation *Installation) error

		// Delete deletes an installation from the datastore.
		Delete(ctx context.Context, installation *Installation) error

		// Find returns installation by ID
		Find(ctx context.Context, id int64) (*Installation, error)

		// FindLogin returns installation by login
		FindLogin(ctx context.Context, login string) (*Installation, error)

		// List returns a slice of installations for a user from the datastore.
		List(ctx context.Context, user *User) ([]*Installation, error)

		// Update persists an updated installation to the datastore.
		Update(ctx context.Context, installation *Installation) error
	}
)
