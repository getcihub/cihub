package main

import (
	"strings"
	"time"

	"github.com/google/wire"
	"github.com/palantir/go-githubapp/githubapp"

	"github.com/getcihub/cihub/cmd/cihub-server/config"
	"github.com/getcihub/cihub/core"
	"github.com/getcihub/cihub/reaper"
	"github.com/getcihub/cihub/service/hook"
	"github.com/getcihub/cihub/service/installation"
	"github.com/getcihub/cihub/service/refresher"
	"github.com/getcihub/cihub/service/runner"
	"github.com/getcihub/cihub/service/user"
	"github.com/getcihub/cihub/session"
	"github.com/getcihub/cihub/trigger"
)

// wire set for loading the services.
var serviceSet = wire.NewSet(
	runner.New,
	user.New,
	trigger.New,

	provideInstallationService,
	provideReaper,
	provideRefresher,
	providerHookParser,
	provideSession,
	provideSystem,
)

// provideInstallationService is a Wire provider function that returns
// an installation service wrapped with a simple cache.
func provideInstallationService(client githubapp.ClientCreator, refresh core.Refresher) core.InstallationService {
	return installation.NewCache(installation.New(client, refresh), 10, time.Minute*5)
}

// providerHookParser is a Wire provider function that returns
// an hook parser.
func providerHookParser(config *config.Config) core.HookParser {
	return hook.New(config.GitHub.App.WebhookSecret)
}

// provideRefresher is a Wire provider function that returns an
// access token refresh based on the environment configuration.
func provideRefresher(store core.UserStore, config *config.Config) core.Refresher {
	return refresher.New(
		store,
		defaultClient(config.GitHub.SkipVerify),
		refresher.NewConfig(
			config.GitHub.OAuth.ClientID,
			config.GitHub.OAuth.ClientSecret,
			strings.TrimSuffix(config.GitHub.Server, "/")+"/login/oauth/access_token",
		),
	)
}

// provideSession is a Wire provider function that returns a
// user session based on the environment configuration.
func provideSession(store core.UserStore, config *config.Config) (core.Session, error) {
	return session.New(store, session.NewConfig(
		config.Session.Secret,
		config.Session.Timeout,
		config.Session.Secure),
	), nil
}

// provideReaper is a Wire provider function that returns a
// zombie runner reaper.
func provideReaper(
	runners core.RunnerStore,
	runnerz core.RunnerService,
	scheduler core.Scheduler,
	config *config.Config,
) *reaper.Reaper {
	return reaper.New(runners, runnerz, scheduler, config.Reaper.Reclaim)
}

// provideSyncer is a Wire provider function that returns the
// system details structure.
func provideSystem(config *config.Config) *core.System {
	return &core.System{
		AppName:        config.GitHub.App.Name,
		InstallationID: config.GitHub.App.IntegrationID,
		Server:         config.GitHub.Server,
	}
}
