package fetch

import (
	"path/filepath"

	"github.com/spf13/cobra"
	"go.chrisrx.dev/x/context"

	quakecontent "github.com/ChrisRx/quake-kube/internal/quake/content"
	quakeserver "github.com/ChrisRx/quake-kube/internal/quake/server"
)

var opts struct {
	AssetsDir  string
	ConfigFile string
}

func NewCommand() *cobra.Command {
	cmd := &cobra.Command{
		Use:          "fetch",
		Short:        "download the content in a server configuration",
		SilenceUsage: true,
		RunE: func(cmd *cobra.Command, args []string) (err error) {
			if !filepath.IsAbs(opts.AssetsDir) {
				opts.AssetsDir, err = filepath.Abs(opts.AssetsDir)
				if err != nil {
					return err
				}
			}
			cfg, err := quakeserver.ReadConfigFromFile(opts.ConfigFile)
			if err != nil {
				return err
			}
			ctx := context.Shutdown()
			defer ctx.Close()
			return quakecontent.Fetch(ctx, opts.AssetsDir, cfg.Game, cfg.Content)
		},
	}
	cmd.Flags().StringVarP(&opts.ConfigFile, "config", "c", "", "server configuration file")
	cmd.Flags().StringVar(&opts.AssetsDir, "assets-dir", "assets", "location for game files")
	return cmd
}
