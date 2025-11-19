package core

import "context"

const (
	// MachineStatusOnline indicates the machine is online and available for jobs
	MachineStatusOnline = "online"

	// MachineStatusOffline indicates the machine is offline (graceful shutdown)
	MachineStatusOffline = "offline"

	// MachineStatusPaused indicates the machine is paused and will not accept new jobs
	MachineStatusPaused = "paused"
)

type (
	// Machine represents a server running an agent.
	Machine struct {
		Name         string `json:"name"`
		Owner        string `json:"owner"`
		Arch         Arch   `json:"arch"`
		CPU          int64  `json:"cpu"`
		CPULimit     int64  `json:"cpu_limit"`
		CPUAllocated int64  `json:"cpu_allocated"`
		RAMAvailable int64  `json:"ram_available"`
		RAMLimit     int64  `json:"ram_limit"`
		RAMAllocated int64  `json:"ram_allocated"`
		RAMTotal     int64  `json:"ram_total"`
		Status       string `json:"status"`
		Created      int64  `json:"created_at"`
		LastSeen     int64  `json:"last_seen_at"`
		Updated      int64  `json:"updated_at"`
		Token        string `json:"-"`
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

// CanAccept checks if a machine can accept a runner for execution.
// A machine can accept a runner if:
//   - The machine is online
//   - The runner owner matches the machine owner
//   - The runner architecture matches the machine architecture
//   - CPU allocation won't exceed the effective CPU limit (CPULimit if set, else CPU)
//   - RAM allocation won't exceed the effective RAM limit (RAMLimit if set, else RAMAvailable)
//   - RAM available on the machine is sufficient for the runner's RAM requirement
func (m *Machine) CanAccept(runner *Runner) bool {
	// Check if machine is online
	if m.Status != MachineStatusOnline {
		return false
	}

	// Check if owner matches
	if m.Owner != runner.Owner {
		return false
	}

	// Check if architecture matches
	if runner.Arch != m.Arch {
		return false
	}

	// Determine effective CPU limit (use CPU total if CPULimit is 0)
	cpuLimit := m.CPULimit
	if cpuLimit == 0 {
		cpuLimit = m.CPU
	}

	// Check if CPU allocation won't exceed limit
	if m.CPUAllocated+runner.CPU > cpuLimit {
		return false
	}

	// Determine effective RAM limit (use RAMAvailable if RAMLimit is 0)
	ramLimit := m.RAMLimit
	if ramLimit == 0 {
		ramLimit = m.RAMAvailable
	}

	// Check if RAM allocation won't exceed limit
	if m.RAMAllocated+runner.RAM > ramLimit {
		return false
	}

	// Check if RAM available on machine is sufficient
	if runner.RAM > m.RAMAvailable {
		return false
	}

	return true
}
