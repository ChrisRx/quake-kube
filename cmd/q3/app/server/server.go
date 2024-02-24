package server

import (
	"context"
	"errors"
	"fmt"
	"net"
	"net/url"
	"path/filepath"
	"time"

	"github.com/soheilhy/cmux"
	"github.com/spf13/cobra"

	quakeclient "github.com/ChrisRx/quake-kube/internal/quake/client"
	quakecontentutil "github.com/ChrisRx/quake-kube/internal/quake/content/util"
	quakeserver "github.com/ChrisRx/quake-kube/internal/quake/server"
	httputil "github.com/ChrisRx/quake-kube/internal/util/net/http"
	"github.com/ChrisRx/quake-kube/pkg/must"
	"github.com/ChrisRx/quake-kube/pkg/mux"
)

var opts struct {
	ClientAddr    string
	ServerAddr    string
	ContentServer string
	AcceptEula    bool
	AssetsDir     string
	ConfigFile    string
	WatchInterval time.Duration
}

func NewCommand() *cobra.Command {
	cmd := &cobra.Command{
		Use:          "server",
		Short:        "q3 server",
		SilenceUsage: true,
		RunE: func(cmd *cobra.Command, args []string) (err error) {
			if !filepath.IsAbs(opts.AssetsDir) {
				opts.AssetsDir, err = filepath.Abs(opts.AssetsDir)
				if err != nil {
					return err
				}
			}

			if !opts.AcceptEula {
				fmt.Println(quakeserver.Q3DemoEULA)
				return errors.New("You must agree to the EULA to continue")
			}

			ctx, cancel := context.WithCancel(context.Background())
			defer cancel()

			// Sync with content server and start ioq3ded server process.
			go func() {
				if err := httputil.GetUntil(opts.ContentServer+"/assets/manifest.json", ctx.Done()); err != nil {
					panic(err)
				}
				// TODO(ChrisRx): only download what is in map config
				if err := quakecontentutil.DownloadAssets(must.Must(url.Parse(opts.ContentServer)), opts.AssetsDir); err != nil {
					panic(err)
				}

				s := quakeserver.Server{
					Dir:           opts.AssetsDir,
					WatchInterval: opts.WatchInterval,
					ConfigFile:    opts.ConfigFile,
					Addr:          opts.ServerAddr,
				}
				if err := s.Start(ctx); err != nil {
					panic(err)
				}
			}()

			m := mux.New(must.Must(net.Listen("tcp", opts.ClientAddr)))
			m.Register(must.Must(quakeclient.NewProxy(ctx, opts.ServerAddr))).
				Match(cmux.HTTP1HeaderField("Upgrade", "websocket"))
			m.Register(must.Must(quakeclient.NewHTTPClientServer(ctx, &quakeclient.Config{
				ContentServerURL: opts.ContentServer,
				ServerAddr:       opts.ServerAddr,
			}))).
				Any()
			fmt.Printf("Starting server %s\n", opts.ClientAddr)
			return m.Serve()
		},
	}
	cmd.Flags().StringVarP(&opts.ConfigFile, "config", "c", "", "server configuration file")
	cmd.Flags().
		StringVar(&opts.ContentServer, "content-server", "http://content.quakejs.com", "content server url")
	cmd.Flags().BoolVar(&opts.AcceptEula, "agree-eula", false, "agree to the Quake 3 demo EULA")
	cmd.Flags().StringVar(&opts.AssetsDir, "assets-dir", "assets", "location for game files")
	cmd.Flags().
		StringVar(&opts.ClientAddr, "client-addr", "0.0.0.0:8080", "client address <host>:<port>")
	cmd.Flags().
		StringVar(&opts.ServerAddr, "server-addr", "0.0.0.0:27960", "dedicated server <host>:<port>")
	cmd.Flags().
		DurationVar(&opts.WatchInterval, "watch-interval", 15*time.Second, "dedicated server <host>:<port>")
	return cmd
}
