package scheduler

import (
	"context"
	"time"

	"github.com/getcihub/cihub/core"
	"github.com/getcihub/cihub/service/redisdb"
)

type scheduler struct {
	*queue
	*canceller
}

type schedulerRedis struct {
	*queue
	*cancellerRedis
}

// New creates a new job scheduler. If Redis client passed as parameter is not nil it uses
// a Redis implementation, otherwise it uses an in-memory implementation.
func New(store core.RunnerStore, r redisdb.RedisDB) core.Scheduler {
	if r == nil {
		return scheduler{
			queue:     newQueue(context.Background(), store),
			canceller: newCanceller(),
		}
	}

	sched := schedulerRedis{
		queue:          newQueue(context.Background(), store),
		cancellerRedis: newCancellerRedis(r),
	}

	const globalMutexExpiryTime = 10 * time.Second
	sched.globMx = r.NewMutex("cihub-scheduler-mx", globalMutexExpiryTime)

	return sched
}
