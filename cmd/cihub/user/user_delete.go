package user

import (
	"context"

	"github.com/urfave/cli/v3"
)

const userDeleteHelp = `
This subcommand deletes a registered user from the system.

Delete the user with login "octocat":

	$ cihub user delete octocat

This subcommand requires administrative privileges.
`

var userDeteleCmd = &cli.Command{
	Name:        "delete",
	Usage:       "Deletes a user by login",
	UsageText:   "cihub user delete [options] LOGIN",
	Description: userDeleteHelp,
	Action:      userDelete,
}

func userDelete(ctx context.Context, cmd *cli.Command) error { return nil }
