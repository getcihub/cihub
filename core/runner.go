package core

import "context"

const (
	RunnerStatusPending   = "pending"
	RunnerStatusIdle      = "idle"
	RunnerStatusBusy      = "busy"
	RunnerStatusCompleted = "completed"
)

type (
	// RunnerParams defines runner query parameters.
	RunnerParams struct {
		After  string // Pagination cursor (runner name)
		Limit  int    // Maximum number of results
		Status string // Filter by status
	}

	// Runner represents a GitHub Actions runner instance.
	// A Runner has an independent lifecycle from a Job. A runner may be
	// created for job A but assigned to job B. Use AssignedTo to link
	// a runner to a specific job.
	Runner struct {
		Name           string `json:"name"`            // Runner registration name
		ID             int64  `json:"id"`              // GitHub runner ID (assigned after registration)
		InstallationID int64  `json:"installation_id"` // GitHub App installation ID for token generation
		Owner          string `json:"owner"`           // GitHub organization name
		Status         string `json:"status"`          // Runner lifecycle status
		AssignedTo     int64  `json:"assigned_to"`     // Job ID this runner is assigned to (0 if idle)
		Busy           bool   `json:"busy"`            // Indicates if runner is busy working
		Cancelled      bool   `json:"cancelled"`       // Cancellation flag
		Completed      int64  `json:"completed"`       // Unix timestamp when runner completed
		Created        int64  `json:"created"`         // Unix timestamp when runner was created
		Started        int64  `json:"started"`         // Unix timestamp when runner started
		Stopped        int64  `json:"stopped"`         // Unix timestamp when runner stopped
		Updated        int64  `json:"updated"`         // Unix timestamp when runner was last updated
		Timeout        int64  `json:"timeout"`         // Runner timeout in seconds
		Token          string `json:"-"`               // Registration token (never logged or exposed)
	}

	RunnerWithToken struct {
		*Runner
		Token string `json:"token"`
	}

	// CreateRunnerOpts defines optional instructions for
	// creating runner instances at the organization level.
	CreateRunnerOpts struct {
		InstallationID int64
		Name           string
		Owner          string
		Labels         []string
		GroupID        int64
	}

	// RunnerStore defines operations for working with runners in a datastore.
	RunnerStore interface {
		// Count returns a count of all runners.
		Count(context.Context) (int64, error)

		// Create persists a new runner to the datastore.
		Create(ctx context.Context, runner *Runner) error

		// Delete deletes a runner from the datastore.
		Delete(ctx context.Context, runner *Runner) error

		// Find returns a runner from the datastore by its name.
		Find(ctx context.Context, name string) (*Runner, error)

		// FindID returns a runner from the datastore by its GitHub runner ID.
		FindID(ctx context.Context, id int64) (*Runner, error)

		// FindAssignedTo returns a runner assigned to a specific job ID.
		FindAssignedTo(ctx context.Context, jobID int64) (*Runner, error)

		// List returns a list of runners from the datastore.
		List(ctx context.Context, params RunnerParams) ([]*Runner, error)

		// ListStatus returns a list of runners filtered by status.
		ListStatus(ctx context.Context, status string) ([]*Runner, error)

		// Purge deletes all stopped runners older than the given unix timestamp.
		Purge(ctx context.Context, before int64) error

		// Update persists an updated runner to the datastore.
		Update(ctx context.Context, runner *Runner) error
	}

	// RunnerService provides access to self-hosted runners from GitHub.
	RunnerService interface {
		// Create creates a new self-hosted runner.
		Create(ctx context.Context, opts CreateRunnerOpts) (*Runner, error)

		// Delete deletes a self-hosted runner.
		Delete(ctx context.Context, runner *Runner) error

		// Find returns a runner for an organization.
		Find(ctx context.Context, owner string, installationID, runnerID int64) (*Runner, error)
	}
)
