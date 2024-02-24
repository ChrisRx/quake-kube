package server

import (
	"context"
	"errors"
	"fmt"
	"log"
	"net"
	"net/url"
	"os"
	"os/signal"
	"path/filepath"
	"syscall"
	"time"

	"github.com/soheilhy/cmux"
	"github.com/spf13/cobra"

	quakeclient "github.com/ChrisRx/quake-kube/internal/quake/client"
	"github.com/ChrisRx/quake-kube/internal/quake/content"
	quakecontentutil "github.com/ChrisRx/quake-kube/internal/quake/content/util"
	quakeserver "github.com/ChrisRx/quake-kube/internal/quake/server"
	. "github.com/ChrisRx/quake-kube/pkg/must"
	"github.com/ChrisRx/quake-kube/pkg/mux"
)

var opts struct {
	ClientAddr     string
	ServerAddr     string
	ContentServer  string
	AcceptEula     bool
	AssetsDir      string
	ConfigFile     string
	WatchInterval  time.Duration
	ShutdownDelay  time.Duration
	SeedContentURL string
}

func NewCommand() *cobra.Command {
	cmd := &cobra.Command{
		Use:          "run",
		Short:        "run QuakeKube",
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

			// Create the assets directory using the embedded game files first, and
			// then any seeded content.
			if err := quakeserver.ExtractGameFiles(opts.AssetsDir); err != nil {
				return err
			}
			if opts.SeedContentURL != "" {
				if err := quakecontentutil.DownloadAssets(Must(url.Parse(opts.SeedContentURL)), opts.AssetsDir); err != nil {
					return err
				}
			}

			ctx, cancel := context.WithCancel(context.Background())
			defer cancel()

			sctx, stop := signal.NotifyContext(ctx, os.Interrupt, syscall.SIGINT, syscall.SIGTERM)
			defer stop()

			qs := quakeserver.Server{
				Addr:          opts.ServerAddr,
				ConfigFile:    opts.ConfigFile,
				Dir:           opts.AssetsDir,
				WatchInterval: opts.WatchInterval,
				ShutdownDelay: opts.ShutdownDelay,
			}
			go func() {
				defer cancel()

				if err := qs.Start(sctx); err != nil {
					log.Printf("quakeserver: %v\n", err)
				}
			}()

			m := mux.New(Must(net.Listen("tcp", opts.ClientAddr)))
			m.Register(content.NewRPCServer(ctx, opts.AssetsDir, opts.ServerAddr)).
				Match(cmux.HTTP2MatchHeaderFieldSendSettings("content-type", "application/grpc"))
			m.Register(content.NewHTTPContentServer(ctx, opts.AssetsDir)).
				Match(cmux.PrefixMatcher("GET /assets"))
			m.Register(Must(quakeclient.NewProxy(ctx, opts.ServerAddr))).
				Match(cmux.HTTP1HeaderField("Upgrade", "websocket"))
			m.Register(Must(quakeclient.NewHTTPClientServer(ctx, &quakeclient.Config{
				ContentServerURL: opts.ContentServer,
				ServerAddr:       opts.ServerAddr,
			}))).
				Any()
			fmt.Printf("Starting server %s\n", opts.ClientAddr)
			return m.ServeAndWait()
		},
	}
	cmd.Flags().StringVarP(&opts.ConfigFile, "config", "c", "", "server configuration file")
	cmd.Flags().StringVar(&opts.ContentServer, "content-server", "http://127.0.0.1:8080", "content server url")
	cmd.Flags().BoolVar(&opts.AcceptEula, "agree-eula", false, "agree to the Quake 3 demo EULA")
	cmd.Flags().StringVar(&opts.AssetsDir, "assets-dir", "assets", "location for game files")
	cmd.Flags().StringVar(&opts.ClientAddr, "client-addr", "0.0.0.0:8080", "client address <host>:<port>")
	cmd.Flags().StringVar(&opts.ServerAddr, "server-addr", "0.0.0.0:27960", "dedicated server <host>:<port>")
	cmd.Flags().DurationVar(&opts.WatchInterval, "watch-interval", 15*time.Second, "watch interval for config file")
	cmd.Flags().DurationVar(&opts.ShutdownDelay, "shutdown-delay", 1*time.Minute, "delay for graceful shutdown")
	cmd.Flags().StringVar(&opts.SeedContentURL, "seed-content-url", "", "seed content from another content server")
	return cmd
}
