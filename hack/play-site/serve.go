package main

import (
	"fmt"
	"net"
	"net/http"
	"os"
	"os/exec"
	"os/signal"
	"runtime"
	"strings"
	"syscall"
)

// serve serves a built site, to try it. Go's file server handles the Range requests the preview
// reads paks with, which simpler ones often don't.
func serve(dir, addr string) error {
	l, err := net.Listen("tcp", addr)
	if err != nil {
		return err
	}
	url := "http://" + addr + "/play/"
	fmt.Printf("Serving %s at http://%s/\n", dir, addr)
	restore := watchKeys(url)
	defer restore()
	return http.Serve(l, http.FileServer(http.Dir(dir)))
}

// watchKeys opens the preview in a browser when the space bar is pressed, returning a function
// that puts the terminal back. It does nothing when stdin isn't a terminal.
func watchKeys(url string) (restore func()) {
	state, err := stty("-g")
	if err != nil {
		return func() {}
	}
	// Keys arrive as they're pressed, without being shown. CTRL+C still stops the server.
	if _, err := stty("-icanon", "-echo", "min", "1"); err != nil {
		return func() {}
	}
	restore = func() {
		_, _ = stty(strings.TrimSpace(state))
	}
	signals := make(chan os.Signal, 1)
	signal.Notify(signals, syscall.SIGINT, syscall.SIGTERM)
	go func() {
		<-signals
		restore()
		os.Exit(130)
	}()
	fmt.Printf("Press space to open %s\n", url)
	go func() {
		key := make([]byte, 1)
		for {
			if _, err := os.Stdin.Read(key); err != nil {
				return
			}
			if key[0] == ' ' {
				if err := openBrowser(url); err != nil {
					fmt.Fprintln(os.Stderr, "play-site: opening a browser:", err)
				}
			}
		}
	}()
	return restore
}

func stty(args ...string) (string, error) {
	cmd := exec.Command("stty", args...)
	cmd.Stdin = os.Stdin
	out, err := cmd.Output()
	return string(out), err
}

func openBrowser(url string) error {
	name := "xdg-open"
	if runtime.GOOS == "darwin" {
		name = "open"
	}
	cmd := exec.Command(name, url)
	if err := cmd.Start(); err != nil {
		return err
	}
	go func() { _ = cmd.Wait() }()
	return nil
}
