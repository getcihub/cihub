package scheduler

import (
	"context"
	"sync"
	"time"

	"github.com/getcihub/cihub/core"
	"github.com/getcihub/cihub/service/redisdb"
)

type queue struct {
	sync.Mutex
	globMx redisdb.LockErr

	ctx      context.Context
	interval time.Duration
	ready    chan struct{}
	store    core.RunnerStore
	workers  map[*worker]struct{}
}

func newQueue(ctx context.Context, store core.RunnerStore) *queue {
	q := &queue{
		store:    store,
		globMx:   redisdb.LockErrNoOp{},
		ready:    make(chan struct{}, 1),
		workers:  map[*worker]struct{}{},
		interval: time.Minute,
		ctx:      ctx,
	}

	go q.start()

	return q
}

func (q *queue) Schedule(ctx context.Context, runner *core.Runner) error {
	select {
	case q.ready <- struct{}{}:
	default:
	}
	return nil
}

func (q *queue) Request(ctx context.Context, params *core.Filter) (*core.Runner, error) {
	ctx, cancel := context.WithCancel(ctx)
	defer cancel()

	w := &worker{
		arch:    params.Arch,
		cpu:     params.CPU,
		owner:   params.Owner,
		ram:     params.RAM,
		channel: make(chan *core.Runner),
		done:    ctx.Done(),
	}

	q.Lock()
	q.workers[w] = struct{}{}
	q.Unlock()

	select {
	case q.ready <- struct{}{}:
	default:
	}

	select {
	case <-ctx.Done():
		q.Lock()
		delete(q.workers, w)
		q.Unlock()
		return nil, ctx.Err()
	case b := <-w.channel:
		return b, nil
	}
}

func (q *queue) signal(ctx context.Context) error {
	err := q.globMx.LockContext(ctx)
	if err != nil {
		return err
	}
	defer q.globMx.UnlockContext(ctx)

	q.Lock()
	count := len(q.workers)
	q.Unlock()

	if count == 0 {
		return nil
	}

	runners, err := q.store.ListPending(ctx)
	if err != nil {
		return err
	}

	q.Lock()
	defer q.Unlock()
	for _, runner := range runners {
		if runner.Machine != "" {
			continue
		}

	loop:
		for w := range q.workers {
			if w.owner != runner.Owner {
				continue
			}
			if w.arch != runner.Arch {
				continue
			}
			if w.cpu < runner.CPU {
				continue
			}
			if w.ram < runner.RAM {
				continue
			}

			sendWork := func() bool {
				select {
				case w.channel <- runner:
					return true
				case <-w.done:
					// Worker will exit when we call the deferred q.Unlock()
				case <-time.After(q.interval):
					// Worker failed to ack before timeout
				}
				return false
			}

			if sendWork() {
				delete(q.workers, w)
				break loop
			}
		}
	}

	return nil
}

func (q *queue) start() error {
	for {
		select {
		case <-q.ctx.Done():
			return q.ctx.Err()
		case <-q.ready:
			q.signal(q.ctx)
		case <-time.After(q.interval):
			q.signal(q.ctx)
		}
	}
}

type worker struct {
	arch    string
	owner   string
	ram     int64
	cpu     int64
	channel chan *core.Runner
	done    <-chan struct{}
}
