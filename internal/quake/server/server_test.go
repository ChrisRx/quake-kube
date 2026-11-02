package server

import (
	"errors"
	"net"
	"os"
	"path/filepath"
	"strings"
	"testing"
)

// fakeConsole answers rcon commands like the server's console, sending what it receives on
// the returned channel.
func fakeConsole(t *testing.T) (string, <-chan string) {
	t.Helper()
	conn, err := net.ListenPacket("udp4", "127.0.0.1:0")
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { conn.Close() })
	commands := make(chan string, 10)
	go func() {
		buf := make([]byte, 65536)
		for {
			n, addr, err := conn.ReadFrom(buf)
			if err != nil {
				return
			}
			commands <- strings.TrimPrefix(string(buf[:n]), "\xff\xff\xff\xff")
			conn.WriteTo([]byte("\xff\xff\xff\xffprint\nexecing quake-kube-live.cfg\n"), addr)
		}
	}()
	return conn.LocalAddr().String(), commands
}

const testConfig = `
server: {password: secret}
commands: ["addbot sarge"]
maps: [{name: q3dm7}]
`

func TestApplyConfig(t *testing.T) {
	addr, commands := fakeConsole(t)
	cfg, err := ParseConfig([]byte(testConfig))
	if err != nil {
		t.Fatal(err)
	}
	s := &Server{Dir: t.TempDir(), game: "baseq3", applied: cfg, rconAddr: addr}
	read := func(name string) string {
		data, _ := os.ReadFile(filepath.Join(s.Dir, "baseq3", name))
		return string(data)
	}

	if err := s.ApplyConfig([]byte(testConfig), "wrong"); !errors.Is(err, ErrUnauthorized) {
		t.Errorf("wrong password: got %v, want ErrUnauthorized", err)
	}
	// The same config changes nothing.
	if err := s.ApplyConfig([]byte(testConfig), "secret"); err != nil {
		t.Fatal(err)
	}
	if read("server.cfg") != "" || len(commands) != 0 {
		t.Error("applying the same config changed the server")
	}

	// A different map is changed from the console, without the commands that only run at
	// startup.
	changed := strings.Replace(testConfig, "q3dm7", "q3dm17", 1)
	if err := s.ApplyConfig([]byte(changed), "secret"); err != nil {
		t.Fatal(err)
	}
	if cmd := <-commands; cmd != "rcon secret exec "+liveConfigName {
		t.Errorf("console got %q", cmd)
	}
	if live := read(liveConfigName); !strings.Contains(live, "map q3dm17") || strings.Contains(live, "addbot") {
		t.Errorf("live config:\n%s", live)
	}
	if !strings.Contains(read("server.cfg"), "addbot sarge") {
		t.Error("server.cfg lost its commands, which a restart runs")
	}
	if s.applied.Maps[0].Name != "q3dm17" {
		t.Errorf("applied map is %s", s.applied.Maps[0].Name)
	}
}

func TestApplyConfigGeneration(t *testing.T) {
	addr, commands := fakeConsole(t)
	cfg, err := ParseConfig([]byte("generation: 3\n" + testConfig))
	if err != nil {
		t.Fatal(err)
	}
	s := &Server{Dir: t.TempDir(), game: "baseq3", applied: cfg, rconAddr: addr}

	// The file catching up with an older config, after the gateway sent a newer one.
	older := "generation: 2\n" + strings.Replace(testConfig, "q3dm7", "q3dm17", 1)
	if err := s.ApplyConfig([]byte(older), "secret"); err != nil {
		t.Fatal(err)
	}
	if len(commands) != 0 || s.applied.Maps[0].Name != "q3dm7" {
		t.Error("applied an older config")
	}

	newer := "generation: 4\n" + strings.Replace(testConfig, "q3dm7", "q3dm17", 1)
	if err := s.ApplyConfig([]byte(newer), "secret"); err != nil {
		t.Fatal(err)
	}
	<-commands
	if s.applied.Generation != 4 || s.applied.Maps[0].Name != "q3dm17" {
		t.Errorf("applied generation %d, map %s", s.applied.Generation, s.applied.Maps[0].Name)
	}
}
