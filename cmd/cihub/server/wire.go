//go:build wireinject

package server

import (
	"github.com/google/wire"

	"github.com/getcihub/cihub/cmd/cihub/server/config"
)

func InitializeApplication(conf *config.Config) (application, error) {
	wire.Build(
		serverSet,
		serviceSet,
		storeSet,
		newApplication,
	)
	return application{}, nil
}
