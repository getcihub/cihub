package main

import (
	"context"
	"flag"
	"os/signal"
	"syscall"

	"github.com/sirupsen/logrus"
	"golang.org/x/sync/errgroup"

	"github.com/getcihub/cihub/cmd/cihub-server/bootstrap"
	"github.com/getcihub/cihub/cmd/cihub-server/config"
	"github.com/getcihub/cihub/core"
	"github.com/getcihub/cihub/orchestrator/agent"
	"github.com/getcihub/cihub/reaper"
	"github.com/getcihub/cihub/server"

	_ "github.com/go-sql-driver/mysql"
	_ "github.com/lib/pq"
	_ "modernc.org/sqlite"
)

func main() {
	var confpath string
	flag.StringVar(&confpath, "c", "./config.toml", "Path to the configuration file")
	flag.Parse()

	config, err := config.Load(confpath)
	if err != nil {
		logrus.Fatalln(err)
	}

	// Set logging level
	logrus.SetLevel(config.Logger.Level)

	ctx, stop := signal.NotifyContext(context.Background(), syscall.SIGINT, syscall.SIGTERM)
	defer stop()

	app, err := InitializeApplication(config)
	if err != nil {
		logrus.WithError(err).
			Fatalln("server: cannot initialize server")
	}

	if len(config.Users) > 0 {
		bootstrapper := bootstrap.New(app.users)
		for _, user := range config.Users {
			if err := bootstrapper.Bootstrap(ctx, &core.User{
				Login: user.Login,
				Admin: user.Admin,
				Token: user.Token,
			}); err != nil {
				logger := logrus.WithError(err)
				logger.Fatalln("server: cannot boostrap user account")
			}
		}
	}

	g := errgroup.Group{}
	g.Go(func() error {
		logrus.WithFields(
			logrus.Fields{
				"host": config.Server.Host,
				"port": config.Server.Port,
				"url":  config.Server.Addr,
				"acme": config.Server.Acme,
			},
		).Infoln("Starting the http server")
		return app.server.ListenAndServe(ctx)
	})

	// launches the reaper in a goroutine.
	//
	// If the reaper is disabled, the goroutine exists immediately
	g.Go(func() error {
		if config.Reaper.Disabled {
			return nil
		}

		logrus.
			WithField("interval", config.Reaper.Interval).
			WithField("reclaim", config.Reaper.Reclaim).
			Infoln("Starting VM reaper")
		return app.reaper.Start(ctx, config.Reaper.Interval)
	})

	// launches the runner agent in a goroutine. If the local
	// agent is disabled (because remote agents are enabled)
	// then the goroutine exits immediately without error.
	g.Go(func() (err error) {
		if app.agent == nil {
			return nil
		}

		logrus.
			WithField("capacity", config.Agent.Capacity).
			WithField("vcpu", config.Agent.VCPU).
			WithField("memory", config.Agent.Memory).
			Infoln("Starting the local runner agent")
		return app.agent.Start(ctx, config.Agent.Capacity)
	})

	if err := g.Wait(); err != nil {
		logrus.WithError(err).Fatalln("program terminated")
	}
}

// application is the main struct for the CIHub server.
type application struct {
	agent  *agent.Agent
	reaper *reaper.Reaper
	server *server.Server
	users  core.UserStore
}

// newApplication returns a new CIHub server application struct.
func newApplication(
	agent *agent.Agent,
	reaper *reaper.Reaper,
	server *server.Server,
	users core.UserStore,
) application {
	return application{
		agent:  agent,
		reaper: reaper,
		server: server,
		users:  users,
	}
}
