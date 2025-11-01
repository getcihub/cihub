package core

import "context"

const (
	// MachineStatusOnline indicates the machine is online and available for jobs
	MachineStatusOnline = "online"

	// MachineStatusOffline indicates the machine is offline (graceful shutdown)
	MachineStatusOffline = "offline"

	// MachineStatusUnhealthy indicates the machine is online but not responding to requests
	MachineStatusUnhealthy = "unhealthy"

	// MachineStatusPaused indicates the machine is paused and will not accept new jobs
	MachineStatusPaused = "paused"
)

type (
	// Machine represents a server running an agent.
	Machine struct {
		Name     string `json:"name"`
		Owner    string `json:"owner"`
		Arch     string `json:"arch"`
		CPU      int64  `json:"cpu"`
		RAM      int64  `json:"ram"`
		Status   string `json:"status"`
		Created  int64  `json:"created_at"`
		LastSeen int64  `json:"last_seen_at"`
		Updated  int64  `json:"updated_at"`
		Token    string `json:"-"`
	}

	// MachineStore defines operations for working with machine on a datastore.
	MachineStore interface {
		// Create persists a new machine to the datastore.
		Create(ctx context.Context, machine *Machine) error

		// Update persists an updated machine to the datastore.
		Update(ctx context.Context, machine *Machine) error

		// Delete deletes a machine from the datastore.
		Delete(ctx context.Context, machine *Machine) error

		// Find returns a machine by hostname and owner.
		Find(ctx context.Context, owner, name string) (*Machine, error)

		// FindToken returns a machine by its authentication token.
		FindToken(ctx context.Context, token string) (*Machine, error)

		// List returns all machines owned by a user.
		List(ctx context.Context, owner string) ([]*Machine, error)

		// Purge deletes offline machines (last_seen older than timestamp).
		Purge(ctx context.Context, before int64) error
	}
)
