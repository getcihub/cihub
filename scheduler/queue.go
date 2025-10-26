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
	store    core.JobStore
	workers  map[*worker]struct{}
}

func newQueue(ctx context.Context, store core.JobStore) *queue {
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

func (q *queue) Schedule(ctx context.Context, job *core.Job) error {
	select {
	case q.ready <- struct{}{}:
	default:
	}
	return nil
}

func (q *queue) Request(ctx context.Context, params *core.Filter) (*core.Job, error) {
	ctx, cancel := context.WithCancel(ctx)
	defer cancel()

	w := &worker{
		arch:    params.Arch,
		memory:  params.Memory,
		vcpu:    params.VCPU,
		channel: make(chan *core.Job),
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

	jobs, err := q.store.ListStatus(ctx, core.JobStatusQueued)
	if err != nil {
		return err
	}

	q.Lock()
	defer q.Unlock()
	for _, job := range jobs {
		if job.Status == core.JobStatusInProgress {
			continue
		}
		if job.Machine != "" {
			continue
		}

	loop:
		for w := range q.workers {
			// job does not match server cpu architecture?
			if w.arch != job.Arch {
				continue
			}

			// job requires more vcpu than available?
			if w.vcpu < job.VCPU {
				continue
			}

			// job requires more memory than available?
			if w.memory < job.Memory {
				continue
			}

			// server pulls job for specific runner?
			if w.owner != "" && w.owner != job.Owner {
				continue
			}

			sendWork := func() bool {
				select {
				case w.channel <- job:
					return true
				case <-w.done:
					// Worker will exit when we call the deferred q.Unlock()
				case <-time.After(q.interval):
					// Worker failed to ack before timeout
				}
				return false
			}

			// job do match, send it and wait for ack
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
	memory  int64
	owner   string
	vcpu    int64
	channel chan *core.Job
	done    <-chan struct{}
}
