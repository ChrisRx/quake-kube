package content

import (
	"net"

	"google.golang.org/grpc"

	contentapiv1 "github.com/ChrisRx/quake-kube/internal/quake/content/api/v1"
	contentapiv2 "github.com/ChrisRx/quake-kube/internal/quake/content/api/v2"
)

type RPCServer struct {
	assetsDir string
}

func NewRPCServer(assetsDir string) *RPCServer {
	return &RPCServer{assetsDir}
}

func (r *RPCServer) Serve(l net.Listener) error {
	s := grpc.NewServer()
	contentapiv1.RegisterAssetsServer(s, contentapiv1.NewAssetsService(r.assetsDir))
	contentapiv2.RegisterAssetsServer(s, contentapiv2.NewAssetsService(r.assetsDir))
	return s.Serve(l)
}
