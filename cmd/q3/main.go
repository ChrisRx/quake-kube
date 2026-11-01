package main

import (
	"log"

	"github.com/spf13/cobra"

	q3fetch "github.com/ChrisRx/quake-kube/cmd/q3/app/fetch"
	q3run "github.com/ChrisRx/quake-kube/cmd/q3/app/run"
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
		q3fetch.NewCommand(),
		q3run.NewCommand(),
	)

	cmd.PersistentFlags().CountVarP(&global.Verbosity, "verbose", "v", "log output verbosity")

	if err := cmd.Execute(); err != nil {
		log.Fatal(err)
	}
}
