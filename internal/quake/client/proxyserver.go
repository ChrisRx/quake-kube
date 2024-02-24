package client

import (
	"bytes"
	"context"
	"fmt"
	"log"
	"net"
	"net/http"
	"time"

	"github.com/gorilla/websocket"
)

var DefaultUpgrader = &websocket.Upgrader{
	ReadBufferSize:  1024,
	WriteBufferSize: 1024,
	CheckOrigin: func(r *http.Request) bool {
		return true
	},
}

// WebsocketUDPProxy
type WebsocketUDPProxy struct {
	Upgrader *websocket.Upgrader

	ctx  context.Context
	addr net.Addr
}

func NewProxy(ctx context.Context, addr string) (*WebsocketUDPProxy, error) {
	host, port, err := net.SplitHostPort(addr)
	if err != nil {
		return nil, err
	}
	proxyTarget := addr
	if net.ParseIP(host).IsUnspecified() {
		// handle case where host is 0.0.0.0
		proxyTarget = net.JoinHostPort("127.0.0.1", port)
	}
	raddr, err := net.ResolveUDPAddr("udp", proxyTarget)
	if err != nil {
		return nil, err
	}
	return &WebsocketUDPProxy{ctx: ctx, addr: raddr}, nil
}

func (w *WebsocketUDPProxy) Serve(l net.Listener) error {
	s := &http.Server{
		Handler: w,
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
	case <-w.ctx.Done():
		return s.Close()
	}
}

func (w *WebsocketUDPProxy) ServeHTTP(rw http.ResponseWriter, req *http.Request) {
	ctx, cancel := context.WithCancel(req.Context())
	defer cancel()

	upgrader := w.Upgrader
	if w.Upgrader == nil {
		upgrader = DefaultUpgrader
	}
	upgradeHeader := http.Header{}
	if hdr := req.Header.Get("Sec-Websocket-Protocol"); hdr != "" {
		upgradeHeader.Set("Sec-Websocket-Protocol", hdr)
	}
	ws, err := upgrader.Upgrade(rw, req, upgradeHeader)
	if err != nil {
		log.Printf("wsproxy: couldn't upgrade %v", err)
		return
	}
	defer ws.Close()

	backend, err := net.ListenPacket("udp", "0.0.0.0:0")
	if err != nil {
		return
	}
	defer backend.Close()

	errc := make(chan error, 1)

	go func() {
		for {
			_, msg, err := ws.ReadMessage()
			if err != nil {
				m := websocket.FormatCloseMessage(websocket.CloseNormalClosure, fmt.Sprintf("%v", err))
				if e, ok := err.(*websocket.CloseError); ok {
					if e.Code != websocket.CloseNoStatusReceived {
						m = websocket.FormatCloseMessage(e.Code, e.Text)
					}
				}
				errc <- err
				ws.WriteMessage(websocket.CloseMessage, m)
				return
			}
			if bytes.HasPrefix(msg, []byte("\xff\xff\xff\xffport")) {
				continue
			}
			if err := backend.SetWriteDeadline(time.Now().Add(5 * time.Second)); err != nil {
				errc <- err
				return
			}
			_, err = backend.WriteTo(msg, w.addr)
			if err != nil {
				errc <- err
				return
			}
		}
	}()

	go func() {
		buffer := make([]byte, 1024*1024)
		for {
			n, _, err := backend.ReadFrom(buffer)
			if err != nil {
				errc <- err
				return
			}
			if err := ws.WriteMessage(websocket.BinaryMessage, buffer[:n]); err != nil {
				errc <- err
				return
			}
		}
	}()

	select {
	case err = <-errc:
		if e, ok := err.(*websocket.CloseError); !ok || e.Code == websocket.CloseAbnormalClosure {
			log.Printf("wsproxy: %v", err)
		}
	case <-ctx.Done():
		return
	}
}
