package redisdb

import (
	"context"
	"fmt"
	"time"

	"github.com/getcihub/cihub/cmd/cihub-server/config"
	"github.com/go-redsync/redsync/v4"
	"github.com/go-redsync/redsync/v4/redis/goredis/v9"
	"github.com/redis/go-redis/v9"
	"github.com/sirupsen/logrus"
)

type PubSubProcessor interface {
	ProcessMessage(s string)
	ProcessError(err error)
}

type RedisDB interface {
	Client() redis.Cmdable
	NewMutex(name string, expiry time.Duration) LockErr
	Subscribe(ctx context.Context, channelName string, channelSize int, proc PubSubProcessor)
}

func New(config config.Config) (RedisDB, error) {
	var (
		err  error
		opts *redis.Options
	)

	if config.Redis.ConnectionString != "" {
		opts, err = redis.ParseURL(config.Redis.ConnectionString)
		if err != nil {
			return nil, err
		}
	} else if config.Redis.Addr != "" {
		opts = &redis.Options{
			Addr:     config.Redis.Addr,
			Password: config.Redis.Password,
			DB:       config.Redis.DB,
		}
	} else {
		return nil, nil
	}

	rdb := redis.NewClient(opts)

	_, err = rdb.Ping(context.Background()).Result()
	if err != nil {
		return nil, fmt.Errorf("redis: not accessible, err: %w", err)
	}

	return &service{
		rdb:      rdb,
		mutexGen: redsync.New(goredis.NewPool(rdb)),
	}, nil
}

type service struct {
	rdb      *redis.Client
	mutexGen *redsync.Redsync
}

func (s *service) Client() redis.Cmdable {
	return s.rdb
}

func (s *service) NewMutex(name string, expiry time.Duration) LockErr {
	var options []redsync.Option
	if expiry > 0 {
		options = append(options, redsync.WithExpiry(expiry))
	}

	return s.mutexGen.NewMutex(name, options...)
}

var backoffDurations = []time.Duration{
	0, time.Second, 3 * time.Second, 5 * time.Second, 10 * time.Second, 20 * time.Second,
}

func (r *service) Subscribe(ctx context.Context, channelName string, channelSize int, proc PubSubProcessor) {
	var connectTry int
	for {
		err := func() (err error) {
			defer func() {
				// panic recovery because external PubSubProcessor methods might cause panics.
				if p := recover(); p != nil {
					err = fmt.Errorf("redisdb pubsub: panic: %v", p)
				}
			}()

			var options []redis.ChannelOption

			if channelSize > 1 {
				options = append(options, redis.WithChannelSize(channelSize))
			}

			pubsub := r.rdb.Subscribe(ctx, channelName)
			ch := pubsub.Channel(options...)

			defer func() {
				_ = pubsub.Close()
			}()

			// make sure the connection is successful
			err = pubsub.Ping(ctx)
			if err != nil {
				return
			}

			connectTry = 0 // successfully connected, reset the counter

			logrus.
				WithField("try", connectTry+1).
				WithField("channel", channelName).
				Trace("redis pubsub: subscribed")

			for {
				select {
				case m, ok := <-ch:
					if !ok {
						err = fmt.Errorf("redis pubsub: channel=%s closed", channelName)
						return
					}

					proc.ProcessMessage(m.Payload)

				case <-ctx.Done():
					err = ctx.Err()
					return
				}
			}
		}()

		if err == nil {
			// should not happen, the function should always exit with an error
			continue
		}

		proc.ProcessError(err)

		if err == context.Canceled || err == context.DeadlineExceeded {
			logrus.
				WithField("channel", channelName).
				Trace("redis pubsub: finished")
			return
		}

		dur := backoffDurations[connectTry]

		logrus.
			WithError(err).
			WithField("try", connectTry+1).
			WithField("pause", dur.String()).
			WithField("channel", channelName).
			Error("redis pubsub: connection failed, reconnecting")

		time.Sleep(dur)

		if connectTry < len(backoffDurations)-1 {
			connectTry++
		}
	}
}
