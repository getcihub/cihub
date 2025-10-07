package core

import "context"

// Client represents a client that is able to interact with a
// remove CIHub server.
type Client interface {
	// Self returns the currently authenticated user.
	Self(ctx context.Context) (*User, error)

	// UserCreate creates a new user account.
	UserCreate(ctx context.Context, user *User) (*User, error)
}
