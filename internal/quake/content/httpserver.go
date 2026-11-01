package content

import (
	"context"
	"encoding/json"
	"io"
	"net"
	"net/http"
	"os"
	"path/filepath"
	"strconv"
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
	e.Use(middleware.RequestLogger())
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
	// Browser clients that load files on demand fetch paks in parts: their indexes, then
	// batches of ranges.
	pakPath := func(c echo.Context) (string, bool) {
		name := filepath.Clean("/" + c.Param("*"))
		path := filepath.Join(assetsDir, name)
		if !strings.HasSuffix(name, ".pk3") {
			return "", false
		}
		if _, err := os.Stat(path); err != nil {
			return "", false
		}
		return path, true
	}
	e.GET("/paks/profile.json", func(c echo.Context) error {
		profile, err := ReadProfiles(assetsDir)
		if err != nil {
			return err
		}
		return c.JSON(http.StatusOK, profile)
	})
	e.GET("/paks/*", func(c echo.Context) error {
		c.SetParamValues(strings.TrimSuffix(c.Param("*"), ".json"))
		path, ok := pakPath(c)
		if !ok {
			return c.String(http.StatusNotFound, "file not found")
		}
		index, err := ReadPakIndex(path)
		if err != nil {
			return err
		}
		return c.JSON(http.StatusOK, index)
	})
	e.POST("/paks/*", func(c echo.Context) error {
		path, ok := pakPath(c)
		if !ok {
			return c.String(http.StatusNotFound, "file not found")
		}
		var ranges [][2]int64
		if err := json.NewDecoder(io.LimitReader(c.Request().Body, 1<<20)).Decode(&ranges); err != nil {
			return c.String(http.StatusBadRequest, err.Error())
		}
		r, size, err := OpenRanges(path, ranges)
		if err != nil {
			return c.String(http.StatusBadRequest, err.Error())
		}
		defer r.Close()
		c.Response().Header().Set(echo.HeaderContentLength, strconv.FormatInt(size, 10))
		return c.Stream(http.StatusOK, echo.MIMEOctetStream, r)
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
