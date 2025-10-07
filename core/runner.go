package core

import "context"

// RunnerStatus represents the operational state of a runner.
type RunnerStatus string

const (
	// RunnerStatusPending indicates the runner is being created.
	RunnerStatusPending = RunnerStatus("pending")

	// RunnerStatusRunning indicates the runner is actively executing.
	RunnerStatusRunning = RunnerStatus("running")

	// RunnerStatusComplete indicates the runner finished successfully.
	RunnerStatusComplete = RunnerStatus("complete")

	// RunnerStatusFailed indicates the runner failed or encountered an error.
	RunnerStatusFailed = RunnerStatus("failed")

	// RunnerStatusCancelled indicates the runner was cancelled.
	RunnerStatusCancelled = RunnerStatus("cancelled")
)

type (
	// Runner represents a GitHub Actions runner instance managed by the orchestrator.
	// Runners are scheduled on nodes and execute GitHub Actions workflows in Firecracker VMs.
	Runner struct {
		ID            int64        `json:"id"`
		Name          string       `json:"name"`
		Org           string       `json:"org"`
		NodeID        int64        `json:"node_id"`
		Status        RunnerStatus `json:"status"`
		Image         string       `json:"image"`
		Memory        int64        `json:"memory"`
		CPUs          int          `json:"cpus"`
		Disk          int64        `json:"disk"`
		RunnerGroupID int          `json:"runner_group_id"`
		Labels        []string     `json:"labels"`
		Created       int64        `json:"created"`
		Started       int64        `json:"started"`
		Stopped       int64        `json:"stopped"`
	}

	// RunnerStore defines operations for working with runners on a datastore.
	RunnerStore interface {
		// Create persists a new runner to the datastore.
		Create(ctx context.Context, runner *Runner) error

		// Delete deletes a runner from the datastore.
		Delete(ctx context.Context, runner *Runner) error

		// Find returns a runner from the datastore by its ID.
		Find(ctx context.Context, id int64) (*Runner, error)

		// FindName returns a runner from the datastore by its name.
		FindName(ctx context.Context, name string) (*Runner, error)

		// List returns a list of runners from the datastore.
		List(ctx context.Context) ([]*Runner, error)

		// ListNode returns a list of runners from the datastore by node ID.
		ListNode(ctx context.Context, nodeID int64) ([]*Runner, error)

		// ListStatus returns a list of runners from the datastore by status.
		ListStatus(ctx context.Context, status RunnerStatus) ([]*Runner, error)

		// Update persists an updated runner to the datastore.
		Update(ctx context.Context, runner *Runner) error
	}

	// RunnerService provides access to GitHub Actions runner operations via the GitHub API.
	RunnerService interface {
		// Register registers a new runner with GitHub for an organization and returns
		// the GitHub runner ID and encoded JIT config token to pass to the runner VM.
		Register(ctx context.Context, runner *Runner) (int64, string, error)

		// Delete removes a runner from GitHub for an organization.
		Delete(ctx context.Context, runner *Runner) error

		// Find returns information about a specific runner from GitHub.
		// This is used to check the runner's status on GitHub before cancelling to avoid desync.
		Find(ctx context.Context, runner *Runner) (*Runner, error)
	}
)
