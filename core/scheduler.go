package core

import "context"

// Filter provides filter criteria to limit jobs requested
// from the scheduler.
type Filter struct {
	Arch  string `json:"arch"`
	CPU   int64  `json:"cpu"`
	Owner string `json:"owner"`
	RAM   int64  `json:"ram"`
}

// Scheduler schedules runners for execution.
type Scheduler interface {
	// Cancel cancels scheduled or running runner.
	Cancel(ctx context.Context, runnerName string) error

	// Cancelled blocks and listens for a cancellation event
	// returning true if the runner has been cancelled.
	Cancelled(ctx context.Context, runnerName string) (bool, error)

	// Request requests the next job scheduled for execution.
	Request(ctx context.Context, params *Filter) (*Runner, error)

	// Schedule schedules the runner for execution.
	Schedule(ctx context.Context, runner *Runner) error
}
