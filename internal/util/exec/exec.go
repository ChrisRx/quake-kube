package exec

import (
	"context"
	"os/exec"
	"syscall"
)

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
