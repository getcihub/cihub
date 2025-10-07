package agent

import (
	"context"

	"github.com/urfave/cli/v3"
)

const help = `
This command starts a CIHub agent responsible for managing
GitHub Actions runners based on provisionning orders. The CIHub
server must be running before use. You must start an agent on
a host that support KVM.

Start an agent with a configuration file:

	$ cihub agent -c /etc/cihub/config.toml
`

var Command = &cli.Command{
	Name:        "agent",
	Usage:       "Start a CIHub agent",
	UsageText:   "cihub agent [option]",
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
