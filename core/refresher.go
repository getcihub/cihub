package core

import "context"

// Refresher refreshes the user account authorization.
type Refresher interface {
	Refresh(ctx context.Context, user *User, force bool) error
}
