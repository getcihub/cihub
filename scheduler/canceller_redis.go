package scheduler

import (
	"context"
	"fmt"
	"sync"
	"time"

	"github.com/redis/go-redis/v9"

	"github.com/getcihub/cihub/service/redisdb"
)

const (
	redisPubSubCancel       = "cihub-cancel"
	redisCancelValuePrefix  = "cihub-cancel-"
	redisCancelValue        = "cancelled"
	redisCancelValueTimeout = time.Minute * 5
)

type cancellerRedis struct {
	sync.Mutex

	rdb         redisdb.RedisDB
	subscribers map[*cancelSubscriber]struct{}
}

type cancelSubscriber struct {
	ch   chan<- error
	name string
}

func newCancellerRedis(r redisdb.RedisDB) *cancellerRedis {
	h := &cancellerRedis{
		rdb:         r,
		subscribers: make(map[*cancelSubscriber]struct{}),
	}

	go r.Subscribe(context.Background(), redisPubSubCancel, 1, h)

	return h
}

func (c *cancellerRedis) Cancel(ctx context.Context, name string) error {
	client := c.rdb.Client()

	// publish a cancel event to all subscribers (agents) watching for it
	_, err := client.Publish(ctx, redisPubSubCancel, name).Result()
	if err != nil {
		return fmt.Errorf("canceller/redis: failed to publish cancellation event, err: %w", err)
	}

	// put a limited duration value in case an agent is not currently listening
	_, err = client.Set(ctx, redisCancelValuePrefix+name, redisCancelValue, redisCancelValueTimeout).Result()
	if err != nil {
		return fmt.Errorf("canceller/redis: failed to set time-to-live, err: %w", err)
	}

	return nil
}

func (c *cancellerRedis) Cancelled(ctx context.Context, name string) (isCancelled bool, err error) {
	client := c.rdb.Client()

	// is the runner already cancelled?
	result, err := client.Get(ctx, redisCancelValuePrefix+name).Result()
	if err != nil && err != redis.Nil {
		return
	}

	isCancelled = err != redis.Nil && result == redisCancelValue
	if isCancelled {
		return
	}

	// if it is not cancelled, subscribe and listen to cancel runner events
	// until the context or runner is cancelled.

	ch := make(chan error)
	sub := &cancelSubscriber{name: name, ch: ch}

	c.Lock()
	c.subscribers[sub] = struct{}{}
	c.Unlock()

	select {
	case err = <-ch:
		// if the runner is cancelled or an error happened,
		// then the subscriber is removed from the set by other goroutine
		isCancelled = err != nil
	case <-ctx.Done():
		// if the context is cancelled then the subscriber must be removed here
		c.Lock()
		delete(c.subscribers, sub)
		c.Unlock()
	}

	return
}

// ProcessMessage informs all subscribers listening to cancellation that the build with this id is cancelled.
// It is a part of redisdb.PubSubProcessor implementation and it's called internally by Subscribe.
func (c *cancellerRedis) ProcessMessage(s string) {
	c.Lock()
	for ss := range c.subscribers {
		if ss.name == s {
			ss.ch <- nil
			close(ss.ch)
			delete(c.subscribers, ss)
		}
	}
	c.Unlock()
}

// ProcessError informs all subscribers that an error happened and clears the set of subscribers.
// The set of subscribers is cleared because each subscriber receives only one message,
// so an error could cause that the message is missed - it's safer to return an error.
// It is a part of redisdb.PubSubProcessor implementation and it's called internally by Subscribe.
func (c *cancellerRedis) ProcessError(err error) {
	c.Lock()
	for ss := range c.subscribers {
		ss.ch <- err
		close(ss.ch)
		delete(c.subscribers, ss)
	}
	c.Unlock()
}
