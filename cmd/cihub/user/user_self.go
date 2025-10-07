package user

import (
	"context"
	"os"
	"text/template"

	"github.com/drone/funcmap"
	"github.com/getcihub/cihub/client"
	"github.com/urfave/cli/v3"
)

const userSelfHelp = `
This subcommand displays the active user.

	$ cihub user self
`

var userSelfCmd = &cli.Command{
	Name:        "self",
	Usage:       "Display active user",
	UsageText:   "cihub user self",
	Description: userSelfHelp,
	Action:      userSelf,
}

func userSelf(ctx context.Context, cmd *cli.Command) error {
	client, err := client.New(cmd)
	if err != nil {
		return err
	}

	user, err := client.Self(context.Background())
	if err != nil {
		return err
	}

	tmpl, err := template.New("_").Funcs(funcmap.Funcs).Parse(tmplUserInfo)
	if err != nil {
		return err
	}
	return tmpl.Execute(os.Stdout, user)
}

// template for user information
var tmplUserInfo = `Authenticated user:
  - Admin: {{ .Admin }}
  - Active: {{ .Active }}
  - Email: {{ .Email }}
  - Login: {{ .Login }}
`
