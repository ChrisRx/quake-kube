package server

import (
	"bytes"
	"context"
	"crypto/subtle"
	_ "embed"
	"errors"
	"fmt"
	"io"
	"log"
	"net"
	"net/http"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"sync"
	"syscall"
	"time"

	"github.com/hako/durafmt"
	"github.com/prometheus/client_golang/prometheus"
	"github.com/prometheus/client_golang/prometheus/promauto"
	"go.chrisrx.dev/x/run"

	"github.com/ChrisRx/quake-kube/internal/quake/content"
	quakenet "github.com/ChrisRx/quake-kube/internal/quake/net"
)

var (
	activePlayers = promauto.NewGauge(prometheus.GaugeOpts{
		Name: "quake_active_players",
		Help: "The current number of active players",
	})

	scores = promauto.NewGaugeVec(prometheus.GaugeOpts{
		Name: "quake_player_scores",
		Help: "Current scores by player, by map",
	}, []string{"player", "map"})

	pings = promauto.NewGaugeVec(prometheus.GaugeOpts{
		Name: "quake_player_pings",
		Help: "Current ping by player",
	}, []string{"player"})

	configReloads = promauto.NewCounter(prometheus.CounterOpts{
		Name: "quake_config_reloads",
		Help: "Config file reload count",
	})
)

type Server struct {
	Addr          string
	ConfigFile    string
	Dir           string
	WatchInterval time.Duration
	ShutdownDelay time.Duration

	cmd *Cmd
	// game is the directory of the game the server runs, where its config is written.
	game string

	// mu guards applied, the config the server is running with, which changes when the config
	// file does or the gateway sends one.
	mu      sync.Mutex
	applied *Config
	// rconAddr is where the server's console is reached from here.
	rconAddr string
	ctx      context.Context
}

// liveConfigName is the config run from the console to change a running server.
const liveConfigName = "quake-kube-live.cfg"

// ErrUnauthorized is returned for a config sent without the server's rcon password.
var ErrUnauthorized = errors.New("unauthorized")

func (s *Server) Start(ctx context.Context) error {
	if s.Addr == "" {
		s.Addr = "0.0.0.0:27960"
	}
	host, port, err := net.SplitHostPort(s.Addr)
	if err != nil {
		return err
	}
	cfg := Default()
	if s.ConfigFile != "" {
		if cfg, err = ReadConfigFromFile(s.ConfigFile); err != nil {
			return err
		}
	}
	// The game can't change while the server runs, so a mod is chosen here rather than in
	// server.cfg.
	s.game = "baseq3"
	if cfg.Game != "" {
		s.game = cfg.Game
	}
	if !content.ValidGameDir(s.game) {
		return fmt.Errorf("invalid game directory %q", s.game)
	}
	args := []string{
		"+set", "dedicated", "2",
		// Only the master in the config is used. Mods such as Urban Terror point empty
		// master slots at their own public masters, so the rest are set to an address
		// nothing answers on.
		"+set", "sv_master1", "", // master.ioquake3.org
		"+set", "sv_master2", "127.0.0.1",
		"+set", "sv_master3", "127.0.0.1",
		"+set", "sv_master4", "127.0.0.1",
		"+set", "sv_master5", "127.0.0.1",
		"+set", "net_ip", host,
		"+set", "net_port", port,
		"+set", "fs_homepath", s.Dir,
		"+set", "com_basegame", "baseq3",
		"+set", "fs_game", cfg.Game,
		"+set", "com_gamename", "Quake3Arena",
		"+exec", "server.cfg",
	}
	// Mods without a qagame.qvm, like Rocket Arena 3, only have a 32-bit x86 library, which
	// only a 32-bit server can load.
	server := "ioq3ded"
	if cfg.Native {
		server = "ioq3ded.i386"
		args = append([]string{"+set", "vm_game", "0"}, args...)
	}
	s.cmd = CommandContext(context.Background(), server, args...)
	if cfg.Native {
		// Libraries that old ask for an executable stack, which glibc no longer allows
		// unless told to.
		s.cmd.Env = append(os.Environ(), "GLIBC_TUNABLES=glibc.rtld.execstack=2")
	}
	s.cmd.Dir = s.Dir
	s.cmd.Stdout = os.Stdout
	s.cmd.Stderr = os.Stderr

	if s.ConfigFile == "" {
		data, err := cfg.Marshal()
		if err != nil {
			return err
		}
		if err := s.writeServerConfig(data); err != nil {
			return err
		}
		if err := s.cmd.Start(); err != nil {
			return err
		}
		return s.cmd.Wait()
	}

	data, err := cfg.Marshal()
	if err != nil {
		return err
	}
	if err := s.writeServerConfig(data); err != nil {
		return err
	}
	if err := s.cmd.Start(); err != nil {
		return err
	}

	go func() {
		if err := s.cmd.Wait(); err != nil {
			log.Println(err)
		}
	}()

	addr := s.Addr
	if net.ParseIP(host).IsUnspecified() {
		addr = net.JoinHostPort("127.0.0.1", port)
	}
	s.mu.Lock()
	s.ctx = ctx
	s.rconAddr = addr
	s.applied = cfg
	s.mu.Unlock()
	go run.Every(ctx, func() {
		status, err := quakenet.GetStatus(addr)
		if err != nil {
			log.Printf("metrics: get status failed %v", err)
			return
		}
		activePlayers.Set(float64(len(status.Players)))
		for _, p := range status.Players {
			if mapname, ok := status.Configuration["mapname"]; ok {
				scores.WithLabelValues(p.Name, mapname).Set(float64(p.Score))
			}
			pings.WithLabelValues(p.Name).Set(float64(p.Ping))
		}
	}, 5*time.Second)

	ch, err := s.watch(ctx)
	if err != nil {
		return err
	}

	// Usually GracefulStop has already stopped it, but Start can also return with an
	// error, e.g. when the config doesn't reload.
	defer s.HardStop()

	for {
		select {
		case <-ch:
			cfg, err := ReadConfigFromFile(s.ConfigFile)
			if err != nil {
				return err
			}
			if err := s.apply(cfg); err != nil {
				return err
			}
		case <-ctx.Done():
			// The shutdown handler has already warned players with GracefulStop.
			return ctx.Err()
		}
	}
}

func (s *Server) GracefulStop() {
	// The server is gone when this returns, however it returns, since the shutdown
	// context is only canceled after it does.
	defer s.HardStop()

	if s.ShutdownDelay == 0 {
		return
	}
	cfg, err := ReadConfigFromFile(s.ConfigFile)
	if err != nil {
		log.Println(err)
		return
	}
	msg := fmt.Sprintf("say SERVER WILL BE SHUTTING DOWN IN %s", strings.ToUpper(durafmt.Parse(s.ShutdownDelay).String()))
	if _, err := quakenet.SendServerCommand(s.Addr, cfg.ServerConfig.Password, msg); err != nil {
		log.Printf("say: %v\n", err)
		return
	}

	ctx, cancel := context.WithTimeout(context.Background(), s.ShutdownDelay)
	defer cancel()

	ticker := time.NewTicker(1 * time.Second)
	defer ticker.Stop()

	// Counts down each second until the delay is up, then says goodbye and kicks everyone.
	countdown := int(s.ShutdownDelay.Seconds())
	for {
		select {
		case <-ticker.C:
			countdown--
			if countdown <= 0 {
				continue
			}
			if _, err := quakenet.SendServerCommand(s.Addr, cfg.ServerConfig.Password, fmt.Sprintf("say %d\n", countdown)); err != nil {
				log.Printf("countdown: %v\n", err)
			}
		case <-ctx.Done():
			if _, err := quakenet.SendServerCommand(s.Addr, cfg.ServerConfig.Password, "say GOODBYE"); err != nil {
				log.Printf("goodbye: %v\n", err)
			}
			status, err := quakenet.GetStatus(s.Addr)
			if err != nil {
				log.Printf("getstatus: %v\n", err)
				return
			}
			for _, player := range status.Players {
				if _, err := quakenet.SendServerCommand(s.Addr, cfg.ServerConfig.Password, fmt.Sprintf("kick %s", player.Name)); err != nil {
					log.Printf("kick: %v\n", err)
				}
			}
			time.Sleep(1 * time.Second)
			return
		}
	}
}

// HardStop kills the server's process and waits briefly for it to exit, so it isn't left
// running when the caller exits right after.
func (s *Server) HardStop() {
	if s.cmd == nil || s.cmd.Process == nil {
		return
	}
	if err := s.cmd.Process.Kill(); err != nil {
		if !errors.Is(err, os.ErrProcessDone) {
			log.Printf("couldn't kill process: %v\n", err)
		}
		return
	}
	// Signal 0 only checks the process exists, which stops being true once it's reaped.
	for range 20 {
		if err := s.cmd.Process.Signal(syscall.Signal(0)); err != nil {
			return
		}
		time.Sleep(100 * time.Millisecond)
	}
	log.Printf("process %d still running after being killed\n", s.cmd.Process.Pid)
}

// apply brings the running server in line with cfg. What the game can change while it runs,
// like the map rotation, game type, limits and bots, is changed from its console, which
// reloads the map without disconnecting anyone. Anything else, like a different mod,
// restarts the server.
func (s *Server) apply(cfg *Config) error {
	s.mu.Lock()
	defer s.mu.Unlock()
	if cfg.Generation < s.applied.Generation {
		log.Printf("ignoring config for generation %d, older than the applied %d\n", cfg.Generation, s.applied.Generation)
		return nil
	}
	data, err := cfg.Marshal()
	if err != nil {
		return err
	}
	current, err := s.applied.Marshal()
	if err != nil {
		return err
	}
	if bytes.Equal(data, current) {
		// E.g. the config file catching up with what the gateway sent.
		s.applied = cfg
		return nil
	}
	if err := s.writeServerConfig(data); err != nil {
		return err
	}
	configReloads.Inc()
	if cfg.Game == s.applied.Game && cfg.Native == s.applied.Native {
		err := s.applyLive(cfg)
		if err == nil {
			log.Println("applied config from the console")
			s.applied = cfg
			return nil
		}
		log.Printf("applying config from the console failed, restarting: %v", err)
	}
	s.applied = cfg
	if err := s.cmd.Restart(s.ctx); err != nil {
		return err
	}
	go func() {
		if err := s.cmd.Wait(); err != nil {
			log.Println(err)
		}
	}()
	return nil
}

// applyLive runs the config from the server's console. Commands are left out, since they're
// only run once the server starts, e.g. adding bots.
func (s *Server) applyLive(cfg *Config) error {
	live := *cfg
	live.Commands = nil
	data, err := live.Marshal()
	if err != nil {
		return err
	}
	if err := s.writeFile(liveConfigName, data); err != nil {
		return err
	}
	resp, err := quakenet.SendServerCommandWithTimeout(s.rconAddr, s.applied.ServerConfig.Password, "exec "+liveConfigName, 2*time.Second)
	if err != nil {
		return err
	}
	if bytes.Contains(resp, []byte("Bad rconpassword")) {
		return errors.New("the console refused the rcon password")
	}
	return nil
}

// ApplyConfig applies a config sent by the gateway, so a change takes effect without waiting
// for the config file to catch up, which can take a minute. Only someone with the server's
// rcon password can send one.
func (s *Server) ApplyConfig(data []byte, password string) error {
	s.mu.Lock()
	applied := s.applied
	s.mu.Unlock()
	if applied == nil {
		return errors.New("the server hasn't started")
	}
	if subtle.ConstantTimeCompare([]byte(password), []byte(applied.ServerConfig.Password)) != 1 {
		return ErrUnauthorized
	}
	cfg, err := ParseConfig(data)
	if err != nil {
		return err
	}
	return s.apply(cfg)
}

// ConfigHandler accepts configs from the gateway, with the rcon password as a bearer token.
func (s *Server) ConfigHandler() http.Handler {
	return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		password, _ := strings.CutPrefix(r.Header.Get("Authorization"), "Bearer ")
		data, err := io.ReadAll(io.LimitReader(r.Body, 1<<20))
		if err != nil {
			http.Error(w, err.Error(), http.StatusBadRequest)
			return
		}
		switch err := s.ApplyConfig(data, password); {
		case errors.Is(err, ErrUnauthorized):
			http.Error(w, err.Error(), http.StatusUnauthorized)
		case err != nil:
			http.Error(w, err.Error(), http.StatusBadRequest)
		default:
			w.WriteHeader(http.StatusNoContent)
		}
	})
}

// writeServerConfig writes server.cfg in the game's directory, which is searched first.
func (s *Server) writeServerConfig(data []byte) error {
	return s.writeFile("server.cfg", data)
}

func (s *Server) writeFile(name string, data []byte) error {
	dir := filepath.Join(s.Dir, s.game)
	if err := os.MkdirAll(dir, 0755); err != nil {
		return err
	}
	return os.WriteFile(filepath.Join(dir, name), data, 0644)
}

func (s *Server) watch(ctx context.Context) (<-chan struct{}, error) {
	if s.WatchInterval == 0 {
		s.WatchInterval = 15 * time.Second
	}
	cur, err := os.Stat(s.ConfigFile)
	if err != nil {
		return nil, err
	}

	ch := make(chan struct{})

	go run.Every(ctx, func() {
		if fi, err := os.Stat(s.ConfigFile); err == nil {
			if fi.ModTime().After(cur.ModTime()) {
				ch <- struct{}{}
			}
			cur = fi
		}
	}, s.WatchInterval)
	return ch, nil
}

type Cmd struct {
	*exec.Cmd
}

func (cmd *Cmd) Restart(ctx context.Context) error {
	if cmd.Process != nil {
		if err := cmd.Process.Kill(); err != nil {
			return err
		}
	}
	newCmd := exec.CommandContext(ctx, cmd.Args[0], cmd.Args[1:]...)
	newCmd.SysProcAttr = cmd.SysProcAttr
	newCmd.Dir = cmd.Dir
	newCmd.Env = cmd.Env
	newCmd.Stdin = cmd.Stdin
	newCmd.Stdout = cmd.Stdout
	newCmd.Stderr = cmd.Stderr
	cmd.Cmd = newCmd
	return cmd.Start()
}

func CommandContext(ctx context.Context, name string, args ...string) *Cmd {
	cmd := exec.CommandContext(ctx, name, args...)
	cmd.SysProcAttr = &syscall.SysProcAttr{
		Setpgid: true,
	}
	return &Cmd{Cmd: cmd}
}
