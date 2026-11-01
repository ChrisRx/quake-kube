package content

import (
	"context"
	"net"
	"net/http"
	"os"
	"path/filepath"
	"strings"
	"time"

	"github.com/labstack/echo/v4"
	"github.com/labstack/echo/v4/middleware"
	"github.com/prometheus/client_golang/prometheus/promhttp"

	quakenet "github.com/ChrisRx/quake-kube/internal/quake/net"
)

type HTTPServer struct {
	*echo.Echo

	ctx context.Context
}

// NewHTTPServer serves what browser clients download from a game server through the
// gateway, the server's content and info, and the server's metrics.
func NewHTTPServer(ctx context.Context, assetsDir, serverAddr string) *HTTPServer {
	e := echo.New()
	e.Use(middleware.Logger())
	e.Use(middleware.Recover())
	e.GET("/assets/manifest.json", func(c echo.Context) error {
		files, err := ReadManifest(assetsDir)
		if err != nil {
			return err
		}
		return c.JSONPretty(http.StatusOK, files, "   ")
	})
	e.GET("/assets/*", func(c echo.Context) error {
		// Only the files in the manifest are served. The server's config, with its rcon
		// password, is in the same directory.
		name := filepath.Clean("/" + trimAssetName(c.Param("*")))
		if !hasExts(name, AssetExts...) {
			return c.String(http.StatusNotFound, "file not found")
		}
		path := filepath.Join(assetsDir, name)
		if _, err := os.Stat(path); os.IsNotExist(err) {
			return c.String(http.StatusNotFound, "file not found")
		}
		return c.File(path)
	})
	e.GET("/info", func(c echo.Context) error {
		m, err := quakenet.GetInfo(serverAddr)
		if err != nil {
			return err
		}
		return c.JSON(http.StatusOK, m)
	})
	e.GET("/metrics", echo.WrapHandler(promhttp.Handler()))
	return &HTTPServer{
		Echo: e,
		ctx:  ctx,
	}
}

func (h *HTTPServer) Serve(l net.Listener) error {
	s := &http.Server{
		Handler:        h,
		ReadTimeout:    5 * time.Minute,
		WriteTimeout:   5 * time.Minute,
		MaxHeaderBytes: 1 << 20,
	}

	errch := make(chan error, 1)
	go func() {
		defer close(errch)

		if err := s.Serve(l); err != nil {
			errch <- err
		}
	}()

	select {
	case err := <-errch:
		return err
	case <-h.ctx.Done():
		return h.Shutdown(h.ctx)
	}
}

// trimAssetName returns a path string that has been prefixed with a crc32
// checksum.
func trimAssetName(s string) string {
	d, f := filepath.Split(s)
	f = f[strings.Index(f, "-")+1:]
	return filepath.Join(d, f)
}

func walkFiles(root string, walkFn filepath.WalkFunc, exts ...string) error {
	return filepath.Walk(root, func(path string, info os.FileInfo, err error) error {
		if err != nil {
			if os.IsPermission(err) {
				return nil
			}
			return err
		}
		if !hasExts(path, exts...) {
			return nil
		}
		if info.IsDir() {
			return nil
		}
		return walkFn(path, info, err)
	})
}

func hasExts(path string, exts ...string) bool {
	for _, ext := range exts {
		if strings.HasSuffix(path, ext) {
			return true
		}
	}
	return false
}
