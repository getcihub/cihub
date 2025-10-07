package manager

import (
	"context"
	"io"
)

type (
	// RunnerInput contains all the configuration and context needed to create
	// and run a GitHub Actions runner VM on an agent node.
	RunnerInput struct {
		CPU     int               `json:"cpu"`
		Env     map[string]string `json:"env"`
		Image   string            `json:"image"`
		Kernel  string            `json:"kernel"`
		RAM     int64             `json:"ram"`
		Storage int64             `json:"storage"`
	}

	// RunnerManager encapsulates complex runner operations on the agent node.
	// The orchestrator calls these methods to manage GitHub Actions runners
	// running in Firecracker VMs. The agent executes the commands but does not
	// store runner state - that is the orchestrator's responsibility.
	RunnerManager interface {
		// Create creates a new GitHub Actions runner VM with the specified configuration.
		// The orchestrator provides all necessary information and is responsible for
		// storing runner metadata.
		Create(ctx context.Context, req *RunnerInput) error

		// Cancel cancels a running GitHub Actions runner VM and stops its execution.
		Cancel(ctx context.Context, name string) error

		// Delete deletes a GitHub Actions runner VM and cleans up its resources.
		Delete(ctx context.Context, name string) error

		// Logs downloads the logs from a runner after it has finished execution.
		// The logs are read from a file on the node and returned as a stream.
		Logs(ctx context.Context, name string) (io.ReadCloser, error)

		// Purge stops and deletes all running runner VMs on this agent.
		Purge(ctx context.Context) error
	}
)

func New() RunnerManager {
	return &manager{}
}

// manager implements the RunnerManager interface.
type manager struct{}

// Create creates a new GitHub Actions runner VM with the specified configuration.
func (m *manager) Create(ctx context.Context, req *RunnerInput) error {
	return nil
}

// Cancel cancels a running GitHub Actions runner VM and stops its execution.
func (m *manager) Cancel(ctx context.Context, name string) error {
	return nil
}

// Delete deletes a GitHub Actions runner VM and cleans up its resources.
func (m *manager) Delete(ctx context.Context, name string) error {
	return nil
}

// Logs downloads the logs from a runner after it has finished execution.
func (m *manager) Logs(ctx context.Context, name string) (io.ReadCloser, error) {
	return nil, nil
}

// Purge stops and deletes all running runner VMs on this agent.
func (m *manager) Purge(ctx context.Context) error {
	return nil
}
