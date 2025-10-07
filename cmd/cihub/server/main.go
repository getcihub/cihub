package server

import (
	"context"
	"os/signal"
	"syscall"

	"github.com/getcihub/cihub/cmd/cihub/server/bootstrap"
	"github.com/getcihub/cihub/cmd/cihub/server/config"
	"github.com/getcihub/cihub/core"
	"github.com/getcihub/cihub/server"
	"github.com/sirupsen/logrus"
	"github.com/urfave/cli/v3"
	"golang.org/x/sync/errgroup"

	_ "github.com/go-sql-driver/mysql"
	_ "github.com/lib/pq"
	_ "github.com/mattn/go-sqlite3"
)

const help = `
This command starts a CIHub server that responds to API
and RPC requests. A server must exist and be reachable by
agents to start GitHub Actions runner.

Start a server with a configuration file:

	$ cihub server -c /etc/cihub/config.toml
`

var Command = &cli.Command{
	Name:        "server",
	Usage:       "Start a CIHub server",
	UsageText:   "cihub server [option]",
	Description: help,
	Action:      runServer,
	Flags: []cli.Flag{
		&cli.StringFlag{
			Name:        "config",
			Usage:       "Path to configuration file",
			Aliases:     []string{"c"},
			DefaultText: "./config.toml",
		},
	},
}

func runServer(ctx context.Context, cmd *cli.Command) error {
	conf, err := config.Load(cmd.String("config"))
	if err != nil {
		logrus.Fatalln(err)
	}

	ctx, stop := signal.NotifyContext(ctx, syscall.SIGINT, syscall.SIGTERM)
	defer stop()

	app, err := InitializeApplication(conf)
	if err != nil {
		logrus.WithError(err).
			Fatalln("server: cannot initialize server")
	}

	if len(conf.Users) > 0 {
		bootstrapper := bootstrap.New(app.users)
		for _, user := range conf.Users {
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
		logrus.
			WithField("acme", conf.Server.Acme).
			WithField("addr", conf.Server.Addr).
			WithField("email", conf.Server.Email).
			WithField("host", conf.Server.Host).
			Infoln("starting the http server")
		return app.server.ListenAndServe(ctx)
	})

	return g.Wait()
}

// application is the main struct for the CIHub server.
type application struct {
	server *server.Server
	users  core.UserStore
}

// newApplication returns a new CIHub server application struct.
func newApplication(server *server.Server, users core.UserStore) application {
	return application{
		server: server,
		users:  users,
	}
}
