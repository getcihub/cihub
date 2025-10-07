package user

import "github.com/urfave/cli/v3"

const userHelp = `
This command groups subcommands for interacting with users.

List all users:

	$ cihub user list

Create a new admin user with login "octocat"

	$ cihub user add octocat --admin

Please see the individual subcommand help for detailed usage information.
`

var Command = &cli.Command{
	Name:        "user",
	Usage:       "Interact with users",
	UsageText:   "cihub user <subcommand> [options] [args]",
	Description: userHelp,
	Commands: []*cli.Command{
		userAddCmd,
		userDeteleCmd,
		userSelfCmd,
	},
}
