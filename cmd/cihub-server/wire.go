//go:build wireinject

package main

import (
	"github.com/google/wire"

	"github.com/getcihub/cihub/cmd/cihub-server/config"
)

func InitializeApplication(conf *config.Config) (application, error) {
	wire.Build(
		clientSet,
		externalSet,
		schedulerSet,
		serverSet,
		serviceSet,
		storeSet,
		newApplication,
	)
	return application{}, nil
}
