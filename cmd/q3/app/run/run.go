package server

import (
	"fmt"
	"log/slog"
	"net"
	"os"
	"path/filepath"
	"time"

	"github.com/labstack/echo/v4"
	"github.com/soheilhy/cmux"
	"github.com/spf13/cobra"
	"go.chrisrx.dev/x/context"
	"go.chrisrx.dev/x/errors"
	"go.chrisrx.dev/x/must"

	"github.com/ChrisRx/quake-kube/internal/mux"
	"github.com/ChrisRx/quake-kube/internal/quake/content"
	quakeserver "github.com/ChrisRx/quake-kube/internal/quake/server"
)

var opts struct {
	ClientAddr    string
	ServerAddr    string
	AcceptEula    bool
	AssetsDir     string
	ConfigFile    string
	WatchInterval time.Duration
	ShutdownDelay time.Duration
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
				fmt.Println(string(quakeserver.Q3DemoEULA))
				return errors.New("You must agree to the EULA to continue")
			}

			// Add the embedded Quake 3 demo files to the content q3 fetch downloaded.
			if err := quakeserver.ExtractGameFiles(opts.AssetsDir); err != nil {
				return err
			}

			ctx := context.Shutdown()
			defer ctx.Close()

			ctx.InterceptTimeout(func() {
				fmt.Println("\rCTRL+C received, press again to shutdown gracefully ...")
			}, 5*time.Second)

			qs := quakeserver.Server{
				Addr:          opts.ServerAddr,
				ConfigFile:    opts.ConfigFile,
				Dir:           opts.AssetsDir,
				WatchInterval: opts.WatchInterval,
				ShutdownDelay: opts.ShutdownDelay,
			}

			context.AfterSignal(ctx, func(sig os.Signal) {
				switch sig {
				case os.Interrupt:
					fmt.Printf("\r%v received, shutting down gracefully, press again to stop now ...\n", sig)
				default:
					fmt.Printf("received %v, shutting down gracefully ...\n", sig)
				}
			})

			ctx.AddHandler(func() {
				fmt.Println("\rCTRL+C pressed again, shutting down immediately ...")
				// We still need to call HardStop to ensure that the underlying child
				// process is killed before exiting.
				qs.HardStop()
				os.Exit(1)
			})

			m := mux.New(must.Ok(net.Listen("tcp", opts.ClientAddr)))
			// The pod's gRPC health checks and the HTTP server share a port.
			m.Register(content.NewRPCServer(ctx, opts.ServerAddr)).
				Match(cmux.HTTP2MatchHeaderFieldSendSettings("content-type", "application/grpc"))
			h := content.NewHTTPServer(ctx, opts.AssetsDir, opts.ServerAddr)
			// The gateway sends changes to the config here, so they apply right away.
			h.POST("/config", echo.WrapHandler(qs.ConfigHandler()))
			m.Register(h).Any()
			fmt.Printf("Starting server %s\n", opts.ClientAddr)
			go func() {
				if err := m.ServeAndWait(); err != nil {
					slog.Error("http server failed", slog.Any("error", err))
				}
			}()

			// Start returns once the server has stopped, gracefully when ctx is canceled.
			return errors.Ignore(qs.Start(ctx), context.Canceled)
		},
	}
	cmd.Flags().StringVarP(&opts.ConfigFile, "config", "c", "", "server configuration file")
	cmd.Flags().BoolVar(&opts.AcceptEula, "agree-eula", false, "agree to the Quake 3 demo EULA")
	cmd.Flags().StringVar(&opts.AssetsDir, "assets-dir", "assets", "location for game files")
	cmd.Flags().StringVar(&opts.ClientAddr, "client-addr", "0.0.0.0:8080", "HTTP and health check address <host>:<port>")
	cmd.Flags().StringVar(&opts.ServerAddr, "server-addr", "0.0.0.0:27960", "dedicated server <host>:<port>")
	cmd.Flags().DurationVar(&opts.WatchInterval, "watch-interval", 15*time.Second, "watch interval for config file")
	cmd.Flags().DurationVar(&opts.ShutdownDelay, "shutdown-delay", 1*time.Minute, "delay for graceful shutdown")
	return cmd
}
