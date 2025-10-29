package main

import (
	"github.com/google/wire"

	"github.com/getcihub/cihub/core"
	"github.com/getcihub/cihub/scheduler"
	"github.com/getcihub/cihub/service/redisdb"
)

// wire set for loading the scheduler.
var schedulerSet = wire.NewSet(
	provideScheduler,
)

// provideScheduler is a Wire provider function that returns a
// scheduler based on the environment configuration.
func provideScheduler(store core.RunnerStore, r redisdb.RedisDB) core.Scheduler {
	return scheduler.New(store, r)
}
