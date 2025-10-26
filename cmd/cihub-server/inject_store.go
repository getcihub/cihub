package main

import (
	"fmt"

	"github.com/google/wire"

	"github.com/getcihub/cihub/cmd/cihub-server/config"
	"github.com/getcihub/cihub/core"
	"github.com/getcihub/cihub/store/job"
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
	provideLabels,
	job.New,
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

// provideLabels is a Wire provider function that provides a map
// of available labels based on environment.
func provideLabels(config *config.Config) (core.Labels, error) {
	m := make(core.Labels, len(config.Labels))
	for _, label := range config.Labels {
		err := label.Validate()
		if err != nil {
			return nil, err
		}

		_, ok := m[label.ID]
		if ok {
			return nil, fmt.Errorf("config: duplicate labels id %s", label.ID)
		}
		m[label.ID] = label
	}

	return m, nil
}
