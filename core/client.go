package core

import "context"

// A Client manages communication with the server.
type Client interface {
	// Join notifies the server the machine is joining the cluster.
	Join(ctx context.Context) error

	// Leave notifies the server the machine is leaving the cluster.
	Leave(ctx context.Context) error

	// Ping sends a ping message to the server to test connectivity.
	Ping(ctx context.Context, resource *Resource) error

	// Request requests the next available runner for execution.
	Request(ctx context.Context) (*RunnerWithToken, error)

	// Accept accepts the runner for execution.
	Accept(ctx context.Context, runner *Runner) error

	// Started signals the runner has started.
	Started(ctx context.Context, runner *Runner) error

	// Lock locks resources for a Runner.
	Lock(ctx context.Context, runner *Runner) error

	// Unlock unlocks resources for a Runner.
	Unlock(ctx context.Context, runner *Runner) error

	// Watch watches the runner for cancellation.
	Watch(ctx context.Context, runner *Runner) (bool, error)
}
