package redisdb

import "context"

// LockErr is an interface with lock and unlock functions that return an error.
// Method names are chosen so that redsync.Mutex implements the interface.
type LockErr interface {
	LockContext(context.Context) error
	UnlockContext(context.Context) (bool, error)
}

// LockErrNoOp is a dummy no-op locker
type LockErrNoOp struct{}

func (l LockErrNoOp) LockContext(context.Context) error           { return nil }
func (l LockErrNoOp) UnlockContext(context.Context) (bool, error) { return false, nil }
