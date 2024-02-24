package mux

import (
	"bytes"
	"fmt"
	"io"
	"log"
	"net"
	"sync"

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
	wg    sync.WaitGroup
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

func (m *Mux) startConns() error {
	for _, c := range m.conns {
		if c.l == nil {
			return fmt.Errorf("match not defined for connection: %T\n", c.conn)
		}
		m.wg.Add(1)
		go func(c *registerConn) {
			defer m.wg.Done()

			if err := c.conn.Serve(c.l); err != nil && err != cmux.ErrListenerClosed {
				log.Println(err)
			}
		}(c)
	}
	return nil
}

// Serve
func (m *Mux) Serve() error {
	if err := m.startConns(); err != nil {
		return err
	}
	return m.CMux.Serve()
}

func (m *Mux) ServeAndWait() error {
	if err := m.startConns(); err != nil {
		return err
	}

	go func() {
		if err := m.CMux.Serve(); err != nil {
			log.Printf("Serve: %v\n", err)
		}
	}()

	m.wg.Wait()
	return nil
}

type registerConn struct {
	*Mux

	conn Connection
	l    net.Listener
}

func (r *registerConn) Match(matches ...any) {
	if len(matches) == 0 {
		return
	}
	if r.l != nil {
		panic(fmt.Errorf("cannot call Match multiple times"))
	}

	switch matches[0].(type) {
	case cmux.Matcher:
		ms := make([]cmux.Matcher, len(matches))
		for i := range matches {
			ms[i] = matches[i].(cmux.Matcher)
		}
		r.l = r.Mux.Match(ms...)
	case cmux.MatchWriter:
		ms := make([]cmux.MatchWriter, len(matches))
		for i := range matches {
			ms[i] = matches[i].(cmux.MatchWriter)
		}
		r.l = r.MatchWithWriters(ms...)
	default:
		panic(fmt.Errorf("expected cmux.Matcher | cmux.MatchWriter, received %T", matches[0]))
	}
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
