package core

import "context"

// Syncer synchronizes the account installation list.
type Syncer interface {
	Sync(context.Context, *User) (*Batch, error)
}
