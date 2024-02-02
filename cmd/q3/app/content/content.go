package content

import (
	"fmt"
	"net"
	"net/url"
	"os"
	"path/filepath"

	"github.com/soheilhy/cmux"
	"github.com/spf13/cobra"

	quakecontent "github.com/ChrisRx/quake-kube/internal/quake/content"
	quakecontentutil "github.com/ChrisRx/quake-kube/internal/quake/content/util"
	"github.com/ChrisRx/quake-kube/pkg/must"
	"github.com/ChrisRx/quake-kube/pkg/mux"
)

var opts struct {
	Addr           string
	AssetsDir      string
	SeedContentURL string
}

func NewCommand() *cobra.Command {
	cmd := &cobra.Command{
		Use:           "content",
		Short:         "q3 content server",
		SilenceErrors: true,
		SilenceUsage:  true,
		RunE: func(cmd *cobra.Command, args []string) (err error) {
			if !filepath.IsAbs(opts.AssetsDir) {
				opts.AssetsDir, err = filepath.Abs(opts.AssetsDir)
				if err != nil {
					return err
				}
			}

			if err := os.MkdirAll(opts.AssetsDir, 0755); err != nil {
				return err
			}

			if opts.SeedContentURL != "" {
				u, err := url.Parse(opts.SeedContentURL)
				if err != nil {
					return err
				}
				if err := quakecontentutil.DownloadAssets(u, opts.AssetsDir); err != nil {
					return err
				}
			}

			m := mux.New(must.Must(net.Listen("tcp", opts.Addr)))
			m.Register(quakecontent.NewRPCServer(opts.AssetsDir)).
				Match(cmux.HTTP2MatchHeaderFieldSendSettings("content-type", "application/grpc"))
			m.Register(quakecontent.NewHTTPContentServer(opts.AssetsDir)).
				Any()
			fmt.Printf("Starting server %s\n", opts.Addr)
			return m.Serve()
		},
	}
	cmd.Flags().StringVarP(&opts.Addr, "addr", "a", ":9090", "address <host>:<port>")
	cmd.Flags().StringVarP(&opts.AssetsDir, "assets-dir", "d", "assets", "assets directory")
	cmd.Flags().StringVar(&opts.SeedContentURL, "seed-content-url", "", "seed content from another content server")
	return cmd
}
