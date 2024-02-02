package main

import (
	"log"

	"github.com/spf13/cobra"

	q3cmd "github.com/ChrisRx/quake-kube/cmd/q3/app/cmd"
	q3content "github.com/ChrisRx/quake-kube/cmd/q3/app/content"
	q3proxy "github.com/ChrisRx/quake-kube/cmd/q3/app/proxy"
	q3server "github.com/ChrisRx/quake-kube/cmd/q3/app/server"
	q3upload "github.com/ChrisRx/quake-kube/cmd/q3/app/upload"
)

var global struct {
	Verbosity int
}

func main() {
	cmd := &cobra.Command{
		Use:   "q3",
		Short: "",
	}
	cmd.AddCommand(
		q3cmd.NewCommand(),
		q3content.NewCommand(),
		q3proxy.NewCommand(),
		q3server.NewCommand(),
		q3upload.NewCommand(),
	)

	cmd.PersistentFlags().CountVarP(&global.Verbosity, "verbose", "v", "log output verbosity")

	if err := cmd.Execute(); err != nil {
		log.Fatal(err)
	}
}
