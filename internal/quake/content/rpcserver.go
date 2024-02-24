package content

import (
	"context"
	"log"
	"net"
	"time"

	"google.golang.org/grpc"
	"google.golang.org/grpc/health"
	healthpb "google.golang.org/grpc/health/grpc_health_v1"

	contentapiv1 "github.com/ChrisRx/quake-kube/internal/quake/content/api/v1"
	contentapiv2 "github.com/ChrisRx/quake-kube/internal/quake/content/api/v2"
	"github.com/ChrisRx/quake-kube/internal/run"
	quakenet "github.com/ChrisRx/quake-kube/pkg/quake/net"
)

type RPCServer struct {
	assetsDir string

	ctx context.Context
	s   *grpc.Server

	serverAddr string
	health     *health.Server
}

func NewRPCServer(ctx context.Context, assetsDir, serverAddr string) *RPCServer {
	r := &RPCServer{
		assetsDir:  assetsDir,
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
	run.Until(func() {
		if _, err := quakenet.SendCommandWithTimeout(r.serverAddr, quakenet.GetInfoCommand, 1*time.Second); err != nil {
			log.Printf("server %q unhealthy: %v\n", r.serverAddr, err)
			r.health.SetServingStatus("", healthpb.HealthCheckResponse_NOT_SERVING)
			return
		}
		r.health.SetServingStatus("", healthpb.HealthCheckResponse_SERVING)
	}, r.ctx.Done(), 10*time.Second)
}

func (r *RPCServer) Serve(l net.Listener) error {
	if r.health != nil {
		r.health.SetServingStatus("", healthpb.HealthCheckResponse_SERVING)
		healthpb.RegisterHealthServer(r.s, r.health)
		go r.checkServerHealth()
	}
	contentapiv1.RegisterAssetsServer(r.s, contentapiv1.NewAssetsService(r.assetsDir))
	contentapiv2.RegisterAssetsServer(r.s, contentapiv2.NewAssetsService(r.assetsDir))

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
