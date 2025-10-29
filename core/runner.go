package core

import "context"

const (
	RunnerStatusPending   = "pending"
	RunnerStatusIdle      = "idle"
	RunnerStatusBusy      = "busy"
	RunnerStatusCompleted = "completed"
)

type (
	// RegisterRunnerOpts defines optional instructions for
	// registering runner instances at the organization level.
	RegisterRunnerOpts struct {
		GroupID        int64
		InstallationID int64
		Labels         []string
		Name           string
		Owner          string
	}

	// Runner represents a GitHub Actions runner.
	Runner struct {
		Name           string   `json:"name"`
		Machine        string   `json:"machine"`
		ID             int64    `json:"id"`
		InstallationID int64    `json:"installation_id"`
		Owner          string   `json:"owner"`
		Status         string   `json:"status"`
		Arch           string   `json:"arch"`
		CPU            int64    `json:"cpu"`
		RAM            int64    `json:"ram"`
		GroupID        int64    `json:"group_id"`
		Labels         []string `json:"labels"`
		Cancelled      int64    `json:"cancelled"`
		Created        int64    `json:"created"`
		Accepted       int64    `json:"accepted"`
		Started        int64    `json:"started"`
		Stopped        int64    `json:"stopped"`
		Updated        int64    `json:"updated"`
		Token          string   `json:"-"`
	}

	RunnerWithToken struct {
		*Runner
		Token string `json:"token"`
	}

	// RunnerManager encapsulates complex runner operations and provides
	// a simplified interface for runner agents.
	RunnerManager interface {
		// Request requests the next available runner that matches
		// machine's capacities. Returns the runner if found, nil if no matching
		// runner available.
		Request(ctx context.Context, params *Filter) (*Runner, error)

		// Accept accepts a runner for execution. This operation uses optimistic
		// locking to prevent multiple agents from executing the same runner.
		Accept(ctx context.Context, name, machine string) error

		// Register registers the runner on GitHub and retrieve its just-in-time
		// configuration.
		Register(ctx context.Context, name string) (*RunnerWithToken, error)

		// Watch watches the runner for cancellation.
		// It returns true if the runner has been cancelled, false otherwise.
		// The agent should call this method periodically during job execution to
		// check for cancellation requests.
		Watch(ctx context.Context, name string) (bool, error)
	}

	// RunnerService provides access to self-hosted runners from GitHub.
	RunnerService interface {
		// Delete deletes a self-hosted runner.
		Delete(ctx context.Context, runner *Runner) error

		// Find returns a runner for an organization.
		Find(ctx context.Context, owner string, installationID, runnerID int64) (*Runner, error)

		// Register registers a new self-hosted runner on GitHub.
		Register(ctx context.Context, opts RegisterRunnerOpts) (*Runner, error)
	}

	// RunnerStore defines operations for working with runners in a datastore.
	RunnerStore interface {
		// Create persists a new runner to the datastore.
		Create(ctx context.Context, runner *Runner) error

		// Delete deletes a runner from the datastore.
		Delete(ctx context.Context, runner *Runner) error

		// Find returns a runner from the datastore by its name.
		Find(ctx context.Context, name string) (*Runner, error)

		// FindID returns a runner from the datastore by its GitHub runner ID.
		FindID(ctx context.Context, id int64) (*Runner, error)

		// ListPending returns a slice of pending runner.
		ListPending(context.Context) ([]*Runner, error)

		// ListIdle returns a slice of idle runner.
		ListIdle(context.Context) ([]*Runner, error)

		// Purge deletes all stopped runners older than the given unix timestamp.
		Purge(ctx context.Context, before int64) error

		// Update persists an updated runner to the datastore.
		Update(ctx context.Context, runner *Runner) error
	}
)
