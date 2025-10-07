package core

import "context"

// Job status values from GitHub workflow_job webhook.
const (
	JobStatusQueued     = "queued"
	JobStatusInProgress = "in_progress"
	JobStatusCompleted  = "completed"
	JobStatusWaiting    = "waiting"
)

// Job conclusion values from GitHub workflow_job webhook.
const (
	JobConclusionSuccess        = "success"
	JobConclusionFailure        = "failure"
	JobConclusionCancelled      = "cancelled"
	JobConclusionSkipped        = "skipped"
	JobConclusionActionRequired = "action_required"
	JobConclusionNeutral        = "neutral"
	JobConclusionTimedOut       = "timed_out"
)

// Job represents a GitHub Actions workflow job received via webhook.
// Jobs are stored in the datastore and wait for the orchestrator to assign them to nodes.
// The job's labels (from runs-on) determine which runner it can be assigned to.
type Job struct {
	ID             int64    `json:"id"`
	RunID          int64    `json:"run_id"`
	Name           string   `json:"name"`
	WorkflowName   string   `json:"workflow_name"`
	Status         string   `json:"status"`
	Conclusion     string   `json:"conclusion"`
	Owner          string   `json:"owner"`
	Repo           string   `json:"repo"`
	Labels         []string `json:"labels"`
	InstallationID int64    `json:"installation_id"`
	RunnerID       int64    `json:"runner_id"`
	Created        int64    `json:"created"`
	Updated        int64    `json:"updated"`
	Started        int64    `json:"started"`
	Completed      int64    `json:"completed"`
}

// JobStore defines operations for working with jobs on a datastore.
type JobStore interface {
	// Create persists a new job to the datastore.
	Create(ctx context.Context, job *Job) error

	// Delete deletes a job from the datastore.
	Delete(ctx context.Context, job *Job) error

	// Find returns a job from the datastore by its GitHub job ID.
	Find(ctx context.Context, id int64) (*Job, error)

	// List returns a list of jobs from the datastore.
	List(ctx context.Context) ([]*Job, error)

	// ListStatus returns a list of jobs from the datastore by status.
	ListStatus(ctx context.Context, status string) ([]*Job, error)

	// ListPending returns all jobs that are queued or waiting for assignment.
	ListPending(ctx context.Context) ([]*Job, error)

	// Update persists an updated job to the datastore.
	Update(ctx context.Context, job *Job) error
}

// JobService provides access to GitHub Actions job operations via the GitHub API.
type JobService interface {
	// Find returns information about a specific job from GitHub.
	Find(ctx context.Context, owner, repo string, jobID int64) (*Job, error)
}
