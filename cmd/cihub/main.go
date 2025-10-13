package main

import (
	"context"
	"log"
	"os"

	"github.com/urfave/cli/v3"

	"github.com/getcihub/cihub/cmd/cihub/agent"
	"github.com/getcihub/cihub/cmd/cihub/server"
	"github.com/getcihub/cihub/version"
)

func main() {
	cmd := &cli.Command{
		Name:                  "cihub",
		Version:               version.Version.String(),
		Usage:                 "Supercharged GitHub Actions runner",
		EnableShellCompletion: true,
		Commands: []*cli.Command{
			agent.Command,
			server.Command,
		},
	}

	if err := cmd.Run(context.Background(), os.Args); err != nil {
		log.Fatal(err)
	}
}
