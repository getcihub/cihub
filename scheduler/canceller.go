package scheduler

import (
	"context"
	"sync"
	"time"
)

type canceller struct {
	sync.Mutex

	cancelled   map[int64]time.Time
	subscribers map[chan struct{}]int64
}

func newCanceller() *canceller {
	return &canceller{
		subscribers: make(map[chan struct{}]int64),
		cancelled:   make(map[int64]time.Time),
	}
}

func (c *canceller) Cancel(ctx context.Context, id int64) error {
	c.Lock()
	defer c.Unlock()

	c.cancelled[id] = time.Now().Add(time.Minute * 5)
	for subscriber, runner := range c.subscribers {
		if id == runner {
			close(subscriber)
		}
	}

	c.collect()

	return nil
}

func (c *canceller) Cancelled(ctx context.Context, id int64) (bool, error) {
	subscriber := make(chan struct{})
	c.Lock()
	c.subscribers[subscriber] = id
	c.Unlock()

	defer func() {
		c.Lock()
		delete(c.subscribers, subscriber)
		c.Unlock()
	}()

	for {
		select {
		case <-ctx.Done():
			return false, ctx.Err()
		case <-time.After(time.Minute):
			c.Lock()
			_, ok := c.cancelled[id]
			c.Unlock()
			if ok {
				return true, nil
			}
		case <-subscriber:
			return true, nil
		}
	}
}

func (c *canceller) collect() {
	// the list of cancelled runners is stored with a ttl, and
	// is not removed until the ttl is reached. This provides
	// adequate window for clients with connectivity issues to
	// reconnect and receive notification of cancel events.
	now := time.Now()
	for runner, timestamp := range c.cancelled {
		if now.After(timestamp) {
			delete(c.cancelled, runner)
		}
	}
}
