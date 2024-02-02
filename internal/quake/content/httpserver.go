package content

import (
	"net"
	"net/http"
	"os"
	"path/filepath"
	"strings"
	"time"

	"github.com/labstack/echo/v4"
	"github.com/labstack/echo/v4/middleware"
	"github.com/soheilhy/cmux"

	contentutil "github.com/ChrisRx/quake-kube/internal/quake/content/util"
)

type HTTPServer struct {
	e *echo.Echo

	assetsDir string
}

func NewHTTPContentServer(assetsDir string) *HTTPServer {
	e := echo.New()
	e.Use(middleware.Logger())
	e.Use(middleware.Recover())
	e.Use(middleware.BodyLimit("1000M"))
	e.Use(middleware.CORSWithConfig(middleware.CORSConfig{
		AllowOrigins: []string{"*"},
		AllowHeaders: []string{echo.HeaderOrigin, echo.HeaderContentType, echo.HeaderAccept},
	}))
	e.GET("/assets/manifest.json", func(c echo.Context) error {
		files, err := contentutil.ReadManifest(assetsDir)
		if err != nil {
			return err
		}
		return c.JSONPretty(http.StatusOK, files, "   ")
	})
	e.GET("/assets/*", func(c echo.Context) error {
		path := filepath.Join(assetsDir, trimAssetName(c.Param("*")))
		if _, err := os.Stat(path); os.IsNotExist(err) {
			return c.String(http.StatusNotFound, "file not found")
		}
		return c.File(path)
	})
	e.GET("/maps", func(c echo.Context) error {
		maps, err := contentutil.ReadMaps(assetsDir)
		if err != nil {
			return err
		}
		return c.JSONPretty(http.StatusOK, maps, "    ")
	})
	return &HTTPServer{
		e:         e,
		assetsDir: assetsDir,
	}
}

func (h *HTTPServer) Match() []cmux.Matcher {
	return []cmux.Matcher{
		cmux.Any(),
	}
}

func (h *HTTPServer) Serve(l net.Listener) error {
	s := &http.Server{
		Handler:        h.e,
		ReadTimeout:    5 * time.Minute,
		WriteTimeout:   5 * time.Minute,
		MaxHeaderBytes: 1 << 20,
	}
	return s.Serve(l)
}

// trimAssetName returns a path string that has been prefixed with a crc32
// checksum.
func trimAssetName(s string) string {
	d, f := filepath.Split(s)
	f = f[strings.Index(f, "-")+1:]
	return filepath.Join(d, f)
}
