package main

import (
	"context"
	"log"
	"os"

	"github.com/getcihub/cihub/cmd/cihub/agent"
	"github.com/getcihub/cihub/cmd/cihub/orchestrator"
	"github.com/getcihub/cihub/cmd/cihub/server"
	"github.com/getcihub/cihub/cmd/cihub/user"
	"github.com/getcihub/cihub/version"
	"github.com/urfave/cli/v3"
)

func main() {
	cmd := &cli.Command{
		Name:                  "cihub",
		Version:               version.Version.String(),
		Usage:                 "Supercharged GitHub Actions runner",
		EnableShellCompletion: true,
		Flags: []cli.Flag{
			&cli.StringFlag{
				Name:    "server",
				Aliases: []string{"s"},
				Usage:   "server address",
				Sources: cli.NewValueSourceChain(cli.EnvVar("CIHUB_SERVER")),
			},
			&cli.StringFlag{
				Name:    "auth-token",
				Aliases: []string{"t"},
				Usage:   "server auth token",
				Sources: cli.NewValueSourceChain(cli.EnvVar("CIHUB_AUTH_TOKEN")),
			},
		},
		Commands: []*cli.Command{
			agent.Command,
			orchestrator.Command,
			server.Command,
			user.Command,
		},
	}

	if err := cmd.Run(context.Background(), os.Args); err != nil {
		log.Fatal(err)
	}
}
