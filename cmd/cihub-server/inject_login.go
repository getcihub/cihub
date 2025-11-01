package main

import (
	"github.com/drone/go-login/login"
	"github.com/drone/go-login/login/github"
	"github.com/google/wire"
	"github.com/sirupsen/logrus"

	"github.com/getcihub/cihub/cmd/cihub-server/config"
)

// wire set for loading the authenticator.
var loginSet = wire.NewSet(
	provideLogin,
)

// provideGithubLogin is a Wire provider function that returns
// a GitHub authenticator based on the environment configuration.
func provideLogin(config *config.Config) login.Middleware {
	if config.GitHub.OAuth.ClientID == "" {
		return nil
	}
	return &github.Config{
		ClientID:     config.GitHub.OAuth.ClientID,
		ClientSecret: config.GitHub.OAuth.ClientSecret,
		Scope:        config.GitHub.OAuth.Scope,
		Server:       config.GitHub.Server,
		Client:       defaultClient(config.GitHub.SkipVerify),
		Logger:       logrus.StandardLogger(),
	}
}
