package main

import (
	"github.com/google/wire"

	"github.com/getcihub/cihub/cmd/cihub-server/config"
	"github.com/getcihub/cihub/service/redisdb"
)

// wire set for loading the external services.
var externalSet = wire.NewSet(
	provideRedisClient,
)

func provideRedisClient(config *config.Config) (redisdb.RedisDB, error) {
	return nil, nil
}
