package content

import (
	"context"
	"log"
	"net"
	"time"

	"go.chrisrx.dev/x/run"
	"google.golang.org/grpc"
	"google.golang.org/grpc/health"
	healthpb "google.golang.org/grpc/health/grpc_health_v1"

	quakenet "github.com/ChrisRx/quake-kube/internal/quake/net"
)

// RPCServer serves the gRPC health service the pod's probes use. The server is healthy while
// the game server answers queries.
type RPCServer struct {
	ctx context.Context
	s   *grpc.Server

	serverAddr string
	health     *health.Server
}

func NewRPCServer(ctx context.Context, serverAddr string) *RPCServer {
	r := &RPCServer{
		serverAddr: serverAddr,
		ctx:        ctx,
		s:          grpc.NewServer(),
	}
	if r.serverAddr != "" {
		r.health = health.NewServer()
	}
	return r
}

func (r *RPCServer) checkServerHealth() {
	run.Every(r.ctx, func() {
		if _, err := quakenet.SendCommandWithTimeout(r.serverAddr, quakenet.GetInfoCommand, 1*time.Second); err != nil {
			log.Printf("server %q unhealthy: %v\n", r.serverAddr, err)
			r.health.SetServingStatus("", healthpb.HealthCheckResponse_NOT_SERVING)
			return
		}
		r.health.SetServingStatus("", healthpb.HealthCheckResponse_SERVING)
	}, 10*time.Second)
}

func (r *RPCServer) Serve(l net.Listener) error {
	if r.health != nil {
		r.health.SetServingStatus("", healthpb.HealthCheckResponse_SERVING)
		healthpb.RegisterHealthServer(r.s, r.health)
		go r.checkServerHealth()
	}

	errch := make(chan error, 1)
	go func() {
		defer close(errch)

		errch <- r.s.Serve(l)
	}()

	select {
	case <-r.ctx.Done():
		r.s.GracefulStop()
		return r.ctx.Err()
	case err := <-errch:
		r.s.GracefulStop()
		return err
	}
}
