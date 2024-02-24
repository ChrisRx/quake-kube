package run

import (
	"errors"
	"log"
	"time"
)

func Until(do func(), stop <-chan struct{}, interval time.Duration) {
	if err := runUntil(func() error {
		do()
		return nil
	}, stop, interval); err != nil {
		log.Println(err)
	}
}

func UntilE(do func() error, stop <-chan struct{}, interval time.Duration) error {
	return runUntil(do, stop, interval)
}

var ErrStopped = errors.New("stopped")

func runUntil(do func() error, stop <-chan struct{}, interval time.Duration) error {
	ticker := time.NewTicker(interval)
	defer ticker.Stop()

	for {
		select {
		case <-ticker.C:
			if err := do(); err != nil {
				return err
			}
		case <-stop:
			return ErrStopped
		}
	}
}
