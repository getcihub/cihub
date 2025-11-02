package machine

import (
	"context"

	"github.com/getcihub/cihub/core"
	"github.com/getcihub/cihub/store/shared/db"
)

type store struct {
	db *db.DB
}

// New returns a new MachineStore.
func New(db *db.DB) core.MachineStore {
	return &store{db}
}

// Create persists a new machine to the datastore.
func (s *store) Create(ctx context.Context, machine *core.Machine) error {
	// TODO: implement
	return nil
}

// Update persists an updated machine to the datastore.
func (s *store) Update(ctx context.Context, machine *core.Machine) error {
	// TODO: implement
	return nil
}

// Delete deletes a machine from the datastore.
func (s *store) Delete(ctx context.Context, machine *core.Machine) error {
	// TODO: implement
	return nil
}

// Find returns a machine by hostname and owner.
func (s *store) Find(ctx context.Context, owner, name string) (*core.Machine, error) {
	// TODO: implement
	return nil, nil
}

// FindToken returns a machine by its authentication token.
func (s *store) FindToken(ctx context.Context, token string) (*core.Machine, error) {
	// TODO: implement
	return nil, nil
}

// List returns all machines owned by a user.
func (s *store) List(ctx context.Context, owner string) ([]*core.Machine, error) {
	// TODO: implement
	return nil, nil
}

// Purge deletes offline machines (last_seen older than timestamp).
func (s *store) Purge(ctx context.Context, before int64) error {
	// TODO: implement
	return nil
}
