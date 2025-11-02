package main

import (
	"github.com/google/wire"

	"github.com/getcihub/cihub/cmd/cihub-server/config"
	"github.com/getcihub/cihub/store/batch"
	"github.com/getcihub/cihub/store/installation"
	"github.com/getcihub/cihub/store/job"
	"github.com/getcihub/cihub/store/machine"
	"github.com/getcihub/cihub/store/membership"
	"github.com/getcihub/cihub/store/runner"
	"github.com/getcihub/cihub/store/shared/db"
	"github.com/getcihub/cihub/store/shared/encrypter"
	"github.com/getcihub/cihub/store/user"
)

// wire set for loading the stores.
//
//nolint:unused
var storeSet = wire.NewSet(
	provideDatabase,
	provideEncrypter,
	installation.New,
	batch.New,
	job.New,
	machine.New,
	membership.New,
	runner.New,
	user.New,
)

// provideDatabase is a Wire provider function that provides a
// database connection.
func provideDatabase(config *config.Config) (*db.DB, error) {
	return db.Connect(
		config.Database.Driver,
		config.Database.Datasource,
		config.Database.MaxConnections,
	)
}

// provideEncrypter is a Wire provider function that provides a
// database encrypter.
func provideEncrypter(config *config.Config) (encrypter.Encrypter, error) {
	return encrypter.New(config.Database.Secret)
}
