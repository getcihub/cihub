package core

import "context"

// Triggerer dis responsible for triggering a Runner from
// an incoming job. If a job is skipped, a nil value is returned.
type Triggerer interface {
	Trigger(context.Context, *Hook) (*Runner, error)
}
