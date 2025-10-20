package main

import (
	"github.com/google/wire"

	"github.com/getcihub/cihub/cmd/cihub-server/config"
	"github.com/getcihub/cihub/core"
	"github.com/getcihub/cihub/service/runner"
	"github.com/getcihub/cihub/session"
)

// wire set for loading the services.
//
//nolint:unused
var serviceSet = wire.NewSet(
	runner.New,
	provideSession,
)

// provideSession is a Wire provider function that returns a
// user session based on the environment configuration.
func provideSession(store core.UserStore, config *config.Config) (core.Session, error) {
	return session.New(store, session.NewConfig(
		config.Session.Secret,
		config.Session.Timeout,
		config.Session.Secure),
	), nil
}
