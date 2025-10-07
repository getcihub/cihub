package orchestrator

import (
	"context"

	"github.com/urfave/cli/v3"
)

const help = `
This command starts a CIHub orchestrator responsible for assigning,
updating, and deleting GitHub Actions runners to a cluster of servers.
The orchestrator manages runner distribution and lifecycle across the
server cluster. The CIHub server must be running before use.

Start an orchestrator with a configuration file:

	$ cihub orchestrator -c /etc/cihub/config.toml
`

var Command = &cli.Command{
	Name:        "orchestrator",
	Usage:       "Start a CIHub orchestrator",
	UsageText:   "cihub orchestrator [option]",
	Description: help,
	Action:      runAgent,
	Flags: []cli.Flag{
		&cli.StringFlag{
			Name:        "config",
			Usage:       "Path to configuration file",
			Aliases:     []string{"c"},
			DefaultText: "./config.toml",
		},
	},
}

func runAgent(ctx context.Context, cmd *cli.Command) error { return nil }
