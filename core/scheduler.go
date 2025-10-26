package core

import "context"

// Filter provides filter criteria to limit jobs requested
// from the scheduler.
type Filter struct {
	Arch   string `json:"arch"`
	Memory int64  `json:"memory"`
	Owner  string `json:"owner"`
	VCPU   int64  `json:"vcpu"`
}

// Scheduler schedules runners for execution.
type Scheduler interface {
	// Schedule schedules the stage for execution.
	Schedule(ctx context.Context, job *Job) error

	// Request requests the next job scheduled for execution.
	Request(ctx context.Context, params *Filter) (*Job, error)

	// Cancel cancels scheduled or running runner.
	Cancel(ctx context.Context, id int64) error

	// Cancelled blocks and listens for a cancellation event
	// returning true if the runner has been cancelled.
	Cancelled(ctx context.Context, id int64) (bool, error)
}
