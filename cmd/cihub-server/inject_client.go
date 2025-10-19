package main

import (
	"fmt"
	"time"

	"github.com/google/wire"
	"github.com/gregjones/httpcache"
	"github.com/palantir/go-githubapp/githubapp"

	"github.com/getcihub/cihub/cmd/cihub-server/config"
	"github.com/getcihub/cihub/version"
)

// wire set for loading the GitHub client.
var clientSet = wire.NewSet(
	provideClient,
)

// provideClient is a Wire provider function that
// returns a GitHub client creator based on the
// environment configuration.
func provideClient(config *config.Config) (githubapp.ClientCreator, error) {
	cc := githubapp.Config{
		WebURL:   config.GitHub.Server,
		V3APIURL: config.GitHub.APIServer,
		V4APIURL: config.GitHub.APIServer,
	}

	// Populate GitHub App configuration
	cc.App.IntegrationID = config.GitHub.App.IntegrationID
	cc.App.WebhookSecret = config.GitHub.App.WebhookSecret
	cc.App.PrivateKey = config.GitHub.App.PrivateKey

	// Populate OAuth configuration
	cc.OAuth.ClientID = config.GitHub.OAuth.ClientID
	cc.OAuth.ClientSecret = config.GitHub.OAuth.ClientSecret

	return githubapp.NewDefaultCachingClientCreator(
		cc,
		githubapp.WithClientUserAgent(fmt.Sprintf("cihub/%s", version.Version)),
		githubapp.WithClientTimeout(time.Second*5),
		githubapp.WithClientCaching(false, func() httpcache.Cache {
			return httpcache.NewMemoryCache()
		}),
	)
}
