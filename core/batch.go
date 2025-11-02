package core

import "context"

// Batch represents a Batch request to synchronize the local
// membership store for a user account.
type Batch struct {
	Insert []*Installation `json:"insert"`
	Update []*Installation `json:"update"`
	Revoke []*Installation `json:"revoke"`
}

// Batcher batch updates the user account.
type Batcher interface {
	Batch(context.Context, *User, *Batch) error
}
