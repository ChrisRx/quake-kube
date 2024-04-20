package run

import (
	"errors"
	"log"
	"time"
)

func Until(fn func(), stop <-chan struct{}, interval time.Duration) {
	if err := runUntil(func() error {
		fn()
		return nil
	}, stop, interval); err != nil {
		log.Println(err)
	}
}

func UntilE(fn func() error, stop <-chan struct{}, interval time.Duration) error {
	return runUntil(fn, stop, interval)
}

var ErrStopped = errors.New("stopped")

func runUntil(fn func() error, stop <-chan struct{}, interval time.Duration) error {
	ticker := time.NewTicker(interval)
	defer ticker.Stop()

	for {
		select {
		case <-ticker.C:
			if err := fn(); err != nil {
				return err
			}
		case <-stop:
			return ErrStopped
		}
	}
}
