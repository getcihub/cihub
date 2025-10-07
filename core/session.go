package core

import "net/http"

// Session provides session management for authenticated users.
type Session interface {
	// Create creates a new user session.
	Create(http.ResponseWriter, *User) error

	// Delete deletes the user session.
	Delete(http.ResponseWriter) error

	// Get returns the user session.
	Get(*http.Request) (*User, error)
}
