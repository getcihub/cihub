package core

import "context"

const (
	// JobStatusQueued indicates a job is waiting to be assigned to a runner.
	JobStatusQueued = "queued"
	// JobStatusInProgress indicates a job is currently running.
	JobStatusInProgress = "in_progress"
	// JobStatusCompleted indicates a job has finished execution.
	JobStatusCompleted = "completed"
	// JobStatusWaiting indicates a job is currently waiting.
	JobStatusWaiting = "waiting"
)

type (
	// JobParams defines job query parameters.
	JobParams struct {
		After  int64  // Pagination cursor (job ID)
		Limit  int    // Maximum number of results
		Status string // Filter by status
	}

	// Job represents a GitHub Actions workflow job.
	// A Job is associated with a Runner when assigned, but they have
	// independent lifecycles. A runner created for job A may be assigned
	// to job B.
	Job struct {
		ID             int64    `json:"id"`              // GitHub workflow job ID
		RunID          int64    `json:"run_id"`          // GitHub workflow run ID
		InstallationID int64    `json:"installation_id"` // GitHub App installation ID
		Owner          string   `json:"owner"`           // Repository owner
		Repo           string   `json:"repo"`            // Repository name
		Workflow       string   `json:"workflow"`        // Workflow name
		Name           string   `json:"name"`            // Job name
		Branch         string   `json:"branch"`          // Branch name where workflow run originated
		SHA            string   `json:"sha"`             // Commit SHA that triggered the workflow run
		Status         string   `json:"status"`          // Job status (queued, in_progress, completed, waiting)
		Conclusion     string   `json:"conclusion"`      // Job conclusion (success, failure, etc.)
		Labels         []string `json:"labels"`          // Required runner labels
		RunnerID       int64    `json:"runner_id"`       // Assigned GitHub runner ID (0 if not assigned)
		RunnerName     string   `json:"runner_name"`     // Assigned runner name (empty if not assigned)
		URL            string   `json:"url"`             // URL to request runner registration token
		OS             string   `json:"os"`              // Operating system image specification (from resolved label)
		Arch           string   `json:"arch"`            // CPU architecture (from resolved label)
		Memory         int64    `json:"memory"`          // Memory in MB (from resolved label)
		VCPU           int64    `json:"vcpu"`            // Virtual CPUs (from resolved label)
		Accepted       int64    `json:"accepted"`        // Unix timestamp when job was accepted by agent
		Queued         int64    `json:"queued"`          // Unix timestamp when job was queued
		Started        int64    `json:"started"`         // Unix timestamp when job started
		Completed      int64    `json:"completed"`       // Unix timestamp when job completed
		Created        int64    `json:"created"`         // Unix timestamp when job record was created
		Updated        int64    `json:"updated"`         // Unix timestamp when job was last updated
		Version        int64    `json:"version"`         // Optimistic locking version
	}

	// JobStore defines operations for working with jobs in a datastore.
	JobStore interface {
		// Count returns a count of all jobs.
		Count(context.Context) (int64, error)

		// Create persists a new job to the datastore.
		Create(ctx context.Context, job *Job) error

		// Delete deletes a job from the datastore.
		Delete(ctx context.Context, job *Job) error

		// Find returns a job from the datastore by its ID.
		Find(ctx context.Context, id int64) (*Job, error)

		// FindRunID returns jobs from the datastore by workflow run ID.
		FindRunID(ctx context.Context, runID int64) ([]*Job, error)

		// List returns a list of jobs from the datastore.
		List(ctx context.Context, params JobParams) ([]*Job, error)

		// ListStatus returns all jobs filtered by status without pagination.
		ListStatus(ctx context.Context, status string) ([]*Job, error)

		// Purge deletes all completed jobs older than the given unix timestamp.
		Purge(ctx context.Context, before int64) error

		// Update persists an updated job to the datastore.
		Update(ctx context.Context, job *Job) error
	}
)
