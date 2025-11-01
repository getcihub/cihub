package core

import "context"

type (
	// Installation represents a GitHub app installation
	Installation struct {
		ID        int64  `json:"id"`
		Owner     string `json:"owner"`
		Personnal bool   `json:"is_personnal"`
		Status    string `json:"status"`
		Created   int64  `json:"created_at"`
		Updated   int64  `json:"updated_at"`
	}

	// InstallationService provides access to installation from GitHub.
	InstallationService interface {
		// List returns a slice of installation the user as access to.
		List(ctx context.Context, user *User) ([]*Installation, error)
	}

	// InstallationStore defines operations for working with installation on a datastore.
	InstallationStore interface {
		// Create persists a new installation to the datastore.
		Create(ctx context.Context, installation *Installation) error

		// Delete deletes an installation from the datastore.
		Delete(ctx context.Context, installation *Installation) error

		// Find returns installation by ID
		Find(ctx context.Context, id int64) (*Installation, error)

		// FindOwner returns installation by owner name
		FindOwner(ctx context.Context, owner string) (*Installation, error)

		// List returns a slice of installations from the datastore.
		List(ctx context.Context) ([]*Installation, error)

		// Update persists an updated installation to the datastore.
		Update(ctx context.Context, installation *Installation) error
	}
)
