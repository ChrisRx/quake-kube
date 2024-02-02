package mux

import (
	"bytes"
	"fmt"
	"io"
	"net"

	"github.com/soheilhy/cmux"
)

// Connection
type Connection interface {
	Serve(l net.Listener) error
}

// Mux
type Mux struct {
	cmux.CMux

	conns []*registerConn
}

// New
func New(l net.Listener) *Mux {
	return &Mux{
		CMux:  cmux.New(l),
		conns: make([]*registerConn, 0),
	}
}

// Register
func (m *Mux) Register(conn Connection) *registerConn {
	r := &registerConn{
		Mux:  m,
		conn: conn,
	}
	m.conns = append(m.conns, r)
	return r
}

// Serve
func (m *Mux) Serve() error {
	for _, conn := range m.conns {
		if !conn.started {
			panic(fmt.Errorf("match not defined for connection: %T\n", conn.conn))
		}
	}
	return m.CMux.Serve()
}

type registerConn struct {
	*Mux

	conn    Connection
	started bool
}

func (r *registerConn) Match(matches ...any) {
	if len(matches) == 0 {
		return
	}

	var l net.Listener
	switch matches[0].(type) {
	case cmux.Matcher:
		ms := make([]cmux.Matcher, len(matches))
		for i := range matches {
			ms[i] = matches[i].(cmux.Matcher)
		}
		l = r.Mux.Match(ms...)
	case cmux.MatchWriter:
		ms := make([]cmux.MatchWriter, len(matches))
		for i := range matches {
			ms[i] = matches[i].(cmux.MatchWriter)
		}
		l = r.MatchWithWriters(ms...)
	default:
		panic(fmt.Errorf("expected cmux.Matcher | cmux.MatchWriter, received %T", matches[0]))
	}

	go func() {
		if err := r.conn.Serve(l); err != cmux.ErrListenerClosed {
			panic(err)
		}
	}()

	r.started = true
}

func (r *registerConn) Any() {
	r.Match(cmux.Any())
}

func URLPrefix(prefix string) cmux.Matcher {
	b := []byte(prefix)
	buf := make([]byte, len(b))
	return func(r io.Reader) bool {
		n, _ := io.ReadFull(r, buf)
		if n >= len(b) {
			n = len(b)
		}
		return bytes.Equal(buf[:n], b)
	}
}
