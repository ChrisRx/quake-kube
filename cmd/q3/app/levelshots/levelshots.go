package levelshots

import (
	"encoding/json"
	"errors"
	"fmt"
	"log/slog"
	"net/http"
	"path/filepath"
	"strings"
	"sync"
	"time"

	"github.com/spf13/cobra"
	"go.chrisrx.dev/x/context"

	quakecontent "github.com/ChrisRx/quake-kube/internal/quake/content"
	quakeserver "github.com/ChrisRx/quake-kube/internal/quake/server"
)

var opts struct {
	Addr       string
	AssetsDir  string
	AcceptEula bool
	ConfigData string
	Maps       []string
	Timeout    time.Duration
}

// The thumbnails' size, at the 4:3 the game shows levelshots at.
const width, height = 256, 192

func NewCommand() *cobra.Command {
	cmd := &cobra.Command{
		Use:   "levelshots",
		Short: "serve thumbnails of the levelshots of a game's maps",
		Long: `Downloads the content in a server configuration, like fetch, and serves a thumbnail
of each map's levelshot as JSON at /levelshots until it's been fetched once. The gateway
runs it for each game it offers, so its menus can show every game's maps.`,
		SilenceUsage: true,
		RunE: func(cmd *cobra.Command, args []string) (err error) {
			if !opts.AcceptEula {
				return errors.New("you must agree to the Quake 3 demo EULA, with --agree-eula")
			}
			if !filepath.IsAbs(opts.AssetsDir) {
				opts.AssetsDir, err = filepath.Abs(opts.AssetsDir)
				if err != nil {
					return err
				}
			}
			cfg, err := quakeserver.ParseConfig([]byte(opts.ConfigData))
			if err != nil {
				return err
			}
			ctx := context.Shutdown()
			defer ctx.Close()

			// Health checks can tell it's working while the content downloads.
			var (
				mu    sync.Mutex
				shots map[string][]byte
				done  = make(chan struct{})
			)
			mux := http.NewServeMux()
			mux.HandleFunc("/healthz", func(w http.ResponseWriter, r *http.Request) {
				mu.Lock()
				defer mu.Unlock()
				if shots == nil {
					http.Error(w, "not ready", http.StatusServiceUnavailable)
				}
			})
			mux.HandleFunc("/levelshots", func(w http.ResponseWriter, r *http.Request) {
				mu.Lock()
				defer mu.Unlock()
				if shots == nil {
					http.Error(w, "not ready", http.StatusServiceUnavailable)
					return
				}
				w.Header().Set("Content-Type", "application/json")
				// []byte is encoded as base64.
				if err := json.NewEncoder(w).Encode(map[string]any{"maps": shots}); err == nil {
					select {
					case <-done:
					default:
						close(done)
					}
				}
			})
			srv := &http.Server{Addr: opts.Addr, Handler: mux}
			go func() {
				if err := srv.ListenAndServe(); err != nil && !errors.Is(err, http.ErrServerClosed) {
					slog.Error("serving levelshots failed", slog.Any("error", err))
				}
			}()
			defer srv.Close()

			if err := quakecontent.Fetch(ctx, opts.AssetsDir, cfg.Game, cfg.Content); err != nil {
				return err
			}
			if err := quakeserver.ExtractGameFiles(opts.AssetsDir); err != nil {
				return err
			}
			result := make(map[string][]byte)
			for _, name := range opts.Maps {
				name = strings.ToLower(name)
				data, err := quakecontent.ReadLevelshot(opts.AssetsDir, name)
				if errors.Is(err, quakecontent.ErrNoLevelshot) {
					slog.Info("map has no levelshot", slog.String("map", name))
					continue
				}
				if err == nil {
					data, err = quakecontent.Thumbnail(data, width, height)
				}
				if err != nil {
					slog.Warn("reading levelshot failed", slog.String("map", name), slog.Any("error", err))
					continue
				}
				result[name] = data
			}
			mu.Lock()
			shots = result
			mu.Unlock()
			fmt.Printf("Serving %d of %d levelshots on %s\n", len(result), len(opts.Maps), opts.Addr)

			select {
			case <-done:
				// Leave time for the response to finish.
				time.Sleep(time.Second)
				return nil
			case <-time.After(opts.Timeout):
				return errors.New("nobody fetched the levelshots")
			case <-ctx.Done():
				return nil
			}
		},
	}
	cmd.Flags().StringVar(&opts.Addr, "addr", "0.0.0.0:8080", "HTTP address <host>:<port>")
	cmd.Flags().StringVar(&opts.AssetsDir, "assets-dir", "assets", "location for game files")
	cmd.Flags().BoolVar(&opts.AcceptEula, "agree-eula", false, "agree to the Quake 3 demo EULA")
	cmd.Flags().StringVar(&opts.ConfigData, "config-data", "", "server configuration, as YAML")
	cmd.Flags().StringSliceVar(&opts.Maps, "maps", nil, "maps to serve levelshots of")
	cmd.Flags().DurationVar(&opts.Timeout, "timeout", 10*time.Minute, "how long to wait for the levelshots to be fetched")
	return cmd
}
