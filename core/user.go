package core

import "context"

type (
	// UserParams defines user query parameters.
	UserParams struct {
		After string
		Limit int
	}

	// User represents a user of the system
	User struct {
		ID      int64  `json:"id"`
		Login   string `json:"login"`
		Email   string `json:"email"`
		Avatar  string `json:"avatar"`
		Active  bool   `json:"active"`
		Admin   bool   `json:"admin"`
		Created int64  `json:"created_at"`
		Updated int64  `json:"updated_at"`
		Access  string `json:"-"`
		Refresh string `json:"-"`
		Expiry  int64  `json:"-"`
		Token   string `json:"-"`
	}

	// UserStore defines operations for working with user on a datastore.
	UserStore interface {
		// Count returns a count of users.
		Count(context.Context) (int64, error)

		// Create persists a new user to the datastore.
		Create(ctx context.Context, user *User) error

		// Delete deletes a user from the datastore.
		Delete(ctx context.Context, user *User) error

		// Find returns a user from the datastore by its ID.
		Find(ctx context.Context, id int64) (*User, error)

		// FindLogin returns a user from the datastore by its login.
		FindLogin(ctx context.Context, login string) (*User, error)

		// FindToken returns a user from the datastore by its token.
		FindToken(ctx context.Context, token string) (*User, error)

		// List returns a list of users from the datastore.
		List(ctx context.Context, params UserParams) ([]*User, error)

		// Update persists an updated user to the datastore.
		Update(ctx context.Context, user *User) error
	}

	// UserService provides access to user account from GitHub.
	UserService interface {
		// Find returns the authenticated user.
		Find(ctx context.Context, access, refresh string) (*User, error)
	}
)
