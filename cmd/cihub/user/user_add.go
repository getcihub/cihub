package user

import (
	"context"
	"fmt"

	"github.com/getcihub/cihub/client"
	"github.com/getcihub/cihub/core"
	"github.com/urfave/cli/v3"
)

const userAddHelp = `
This subcommand registers a new user with the system.

Add a user with login "octocat":

	$ cihub user add octocat

Add a user with login "octocat" and admin privilege:

	$ cihub user add --admin octocat

This subcommand requires administrative privileges.
`

var userAddFlags = []cli.Flag{
	&cli.BoolFlag{
		Name:  "admin",
		Usage: "admin privileged",
	},
	&cli.StringFlag{
		Name:  "token",
		Usage: "api token",
	},
}

var userAddCmd = &cli.Command{
	Name:        "add",
	Usage:       "Adds a new user",
	UsageText:   "cihub user add [options] LOGIN",
	Description: userAddHelp,
	Action:      userAdd,
	Flags:       userAddFlags,
}

func userAdd(ctx context.Context, cmd *cli.Command) error {
	login := cmd.Args().First()
	if login == "" {
		return fmt.Errorf("must provide a valid login")
	}

	client, err := client.New(cmd)
	if err != nil {
		return err
	}

	in := &core.User{
		Login: login,
		Admin: cmd.Bool("admin"),
		Token: cmd.String("token"),
	}

	user, err := client.UserCreate(context.Background(), in)
	if err != nil {
		return err
	}

	fmt.Printf("Successfully added user %s\n", user.Login)
	if user.Token != "" {
		fmt.Printf("Generated account token %s\n", user.Token)
	}

	return nil
}
