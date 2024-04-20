package server

import (
	"context"
	_ "embed"
	"fmt"
	"log"
	"net"
	"os"
	"path/filepath"
	"strings"
	"time"

	"github.com/hako/durafmt"
	"github.com/prometheus/client_golang/prometheus"
	"github.com/prometheus/client_golang/prometheus/promauto"

	"github.com/ChrisRx/quake-kube/internal/run"
	"github.com/ChrisRx/quake-kube/internal/util/exec"
	quakenet "github.com/ChrisRx/quake-kube/pkg/quake/net"
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

	cmd *exec.Cmd
}

func (s *Server) Start(ctx context.Context) error {
	if s.Addr == "" {
		s.Addr = "0.0.0.0:27960"
	}
	host, port, err := net.SplitHostPort(s.Addr)
	if err != nil {
		return err
	}
	args := []string{
		"+set", "dedicated", "2",
		"+set", "sv_master1", "", // master.ioquake3.org
		"+set", "sv_master2", "", // master.quake3arena..com
		"+set", "sv_master3", "", // localhost:27950
		"+set", "net_ip", host,
		"+set", "net_port", port,
		"+set", "fs_homepath", s.Dir,
		"+set", "com_basegame", "baseq3",
		// This won't work with the q3demo pak files:
		// "+set", "fs_game", "arena",
		"+set", "com_gamename", "Quake3Arena",
		"+exec", "server.cfg",
	}
	s.cmd = exec.CommandContext(context.Background(), "ioq3ded", args...)
	s.cmd.Dir = s.Dir
	s.cmd.Stdout = os.Stdout
	s.cmd.Stderr = os.Stderr

	if s.ConfigFile == "" {
		cfg := Default()
		data, err := cfg.Marshal()
		if err != nil {
			return err
		}
		if err := os.WriteFile(filepath.Join(s.Dir, "baseq3/server.cfg"), data, 0644); err != nil {
			return err
		}
		if err := s.cmd.Start(); err != nil {
			return err
		}
		return s.cmd.Wait()
	}

	if err := s.reload(); err != nil {
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

	go func() {
		addr := s.Addr
		if net.ParseIP(host).IsUnspecified() {
			addr = net.JoinHostPort("127.0.0.1", port)
		}
		tick := time.NewTicker(5 * time.Second)
		defer tick.Stop()

		for {
			select {
			case <-tick.C:
				status, err := quakenet.GetStatus(addr)
				if err != nil {
					log.Printf("metrics: get status failed %v", err)
					continue
				}
				activePlayers.Set(float64(len(status.Players)))
				for _, p := range status.Players {
					if mapname, ok := status.Configuration["mapname"]; ok {
						scores.WithLabelValues(p.Name, mapname).Set(float64(p.Score))
					}
					pings.WithLabelValues(p.Name).Set(float64(p.Ping))
				}
			case <-ctx.Done():
				return
			}
		}
	}()

	ch, err := s.watch(ctx)
	if err != nil {
		return err
	}

	defer func() {
		if s.cmd.Process != nil {
			if err := s.cmd.Process.Kill(); err != nil {
				log.Printf("couldn't kill process: %v\n", err)
			}
		}
	}()

	for {
		select {
		case <-ch:
			if err := s.reload(); err != nil {
				return err
			}
			configReloads.Inc()
			if err := s.cmd.Restart(ctx); err != nil {
				return err
			}
			go func() {
				if err := s.cmd.Wait(); err != nil {
					log.Println(err)
				}
			}()
		case <-ctx.Done():
			s.GracefulStop()
			return ctx.Err()
		}
	}
}

func (s *Server) GracefulStop() {
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

	countdown := int(s.ShutdownDelay.Seconds()) - 1
	for {
		select {
		case <-ticker.C:
			countdown--
			if countdown == 0 {
				return
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

func (s *Server) HardStop() {
	if s.cmd.Process != nil {
		if err := s.cmd.Process.Kill(); err != nil {
			log.Printf("couldn't kill process: %v\n", err)
		}
	}
}

func (s *Server) reload() error {
	cfg, err := ReadConfigFromFile(s.ConfigFile)
	if err != nil {
		return err
	}
	data, err := cfg.Marshal()
	if err != nil {
		return err
	}
	return os.WriteFile(filepath.Join(s.Dir, "baseq3/server.cfg"), data, 0644)
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

	go run.Until(func() {
		if fi, err := os.Stat(s.ConfigFile); err == nil {
			if fi.ModTime().After(cur.ModTime()) {
				ch <- struct{}{}
			}
			cur = fi
		}
	}, ctx.Done(), s.WatchInterval)
	return ch, nil
}
