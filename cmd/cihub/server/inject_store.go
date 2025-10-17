package server

import (
	"github.com/google/wire"

	"github.com/getcihub/cihub/cmd/cihub/server/config"
	"github.com/getcihub/cihub/core"
	"github.com/getcihub/cihub/store/label"
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
	provideLabelStore,
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

// provideLabelStore is a Wire provider function that provides an
// in-memory label store loaded from configuration.
func provideLabelStore(config *config.Config) (core.LabelStore, error) {
	labels := make([]*core.Label, 0, len(config.Labels))
	for _, l := range config.Labels {
		labels = append(labels,
			&core.Label{
				Name:    l.Name,
				CPU:     l.CPU,
				RAM:     l.RAM,
				Storage: l.Storage,
				Kernel:  l.Kernel,
				Ubuntu:  l.Ubuntu,
			},
		)
	}

	return label.New(labels)
}
