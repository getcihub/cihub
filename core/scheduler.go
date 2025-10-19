package core

import "context"

// Scheduler schedules runners for execution.
type Scheduler interface {
	// Request requests the next job scheduled for execution.
	Request(ctx context.Context, labels []string) (*Job, error)

	// Cancel cancels scheduled or running runner.
	Cancel(ctx context.Context, id int64) error

	// Cancelled blocks and listens for a cancellation event
	// returning true if the runner has been cancelled.
	Cancelled(ctx context.Context, id int64) (bool, error)
}
