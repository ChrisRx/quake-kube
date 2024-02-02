package client

import (
	"html/template"
	"io"
	"net"
	"net/http"
	"net/url"
	"time"

	"github.com/labstack/echo/v4"
	"github.com/labstack/echo/v4/middleware"
	"github.com/prometheus/client_golang/prometheus/promhttp"
	"github.com/soheilhy/cmux"

	quakenet "github.com/ChrisRx/quake-kube/pkg/quake/net"
)

type Config struct {
	ContentServerURL string
	ServerAddr       string
}

type HTTPClientServer struct {
	cfg *Config
	e   *echo.Echo
}

func NewHTTPClientServer(cfg *Config) (*HTTPClientServer, error) {
	e := echo.New()
	e.Use(middleware.Logger())
	e.Use(middleware.Recover())
	e.Use(middleware.CORSWithConfig(middleware.CORSConfig{
		AllowOrigins: []string{"*"},
		AllowHeaders: []string{echo.HeaderOrigin, echo.HeaderContentType, echo.HeaderAccept},
	}))

	static, err := LoadStaticFiles()
	if err != nil {
		return nil, err
	}
	data, err := static.ReadFile("index.html")
	if err != nil {
		return nil, err
	}
	templates, err := template.New("index").Parse(string(data))
	if err != nil {
		return nil, err
	}
	e.Renderer = &TemplateRenderer{templates}

	e.GET("/", func(c echo.Context) error {
		m, err := quakenet.GetInfo(cfg.ServerAddr)
		if err != nil {
			return err
		}
		needsPass := false
		if v, ok := m["g_needpass"]; ok {
			if v == "1" {
				needsPass = true
			}
		}
		return c.Render(http.StatusOK, "index", map[string]interface{}{
			"ServerAddr": cfg.ServerAddr,
			"NeedsPass":  needsPass,
		})
	})

	e.GET("/metrics", echo.WrapHandler(promhttp.Handler()))

	e.GET("/info", func(c echo.Context) error {
		m, err := quakenet.GetInfo(cfg.ServerAddr)
		if err != nil {
			return err
		}
		return c.JSON(http.StatusOK, m)
	})

	e.GET("/status", func(c echo.Context) error {
		m, err := quakenet.GetStatus(cfg.ServerAddr)
		if err != nil {
			return err
		}
		return c.JSON(http.StatusOK, m)
	})

	e.GET("/*", echo.WrapHandler(http.FileServer(static)))

	// Quake3 assets requests must be proxied to the content server. The host
	// header is manipulated to ensure that services like CloudFlare will not
	// reject requests based upon incorrect host header.
	csurl, err := url.Parse(cfg.ContentServerURL)
	if err != nil {
		return nil, err
	}
	g := e.Group("/assets")
	g.Use(middleware.ProxyWithConfig(middleware.ProxyConfig{
		Balancer: middleware.NewRoundRobinBalancer([]*middleware.ProxyTarget{
			{URL: csurl},
		}),
		Transport: &HostHeaderTransport{RoundTripper: http.DefaultTransport, Host: csurl.Host},
	}))
	return &HTTPClientServer{
		cfg: cfg,
		e:   e,
	}, nil
}

func (h *HTTPClientServer) Match() []cmux.Matcher {
	return []cmux.Matcher{
		cmux.Any(),
	}
}

func (h *HTTPClientServer) Serve(l net.Listener) error {
	s := &http.Server{
		Handler:        h.e,
		ReadTimeout:    5 * time.Minute,
		WriteTimeout:   5 * time.Minute,
		MaxHeaderBytes: 1 << 20,
	}
	return s.Serve(l)
}

type HostHeaderTransport struct {
	http.RoundTripper
	Host string
}

func (t *HostHeaderTransport) RoundTrip(req *http.Request) (*http.Response, error) {
	req.Host = t.Host
	return t.RoundTripper.RoundTrip(req)
}

type TemplateRenderer struct {
	*template.Template
}

func (t *TemplateRenderer) Render(
	w io.Writer,
	name string,
	data interface{},
	c echo.Context,
) error {
	return t.ExecuteTemplate(w, name, data)
}
