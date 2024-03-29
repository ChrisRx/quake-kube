// Code generated by protoc-gen-go-grpc. DO NOT EDIT.
// versions:
// - protoc-gen-go-grpc v1.3.0
// - protoc             v4.24.4
// source: content/v1/assets.proto

package v1

import (
	context "context"
	grpc "google.golang.org/grpc"
	codes "google.golang.org/grpc/codes"
	status "google.golang.org/grpc/status"
)

// This is a compile-time assertion to ensure that this generated file
// is compatible with the grpc package it is being compiled against.
// Requires gRPC-Go v1.32.0 or later.
const _ = grpc.SupportPackageIsVersion7

const (
	Assets_FileUpload_FullMethodName = "/content.v1.Assets/FileUpload"
)

// AssetsClient is the client API for Assets service.
//
// For semantics around ctx use and closing/ending streaming RPCs, please refer to https://pkg.go.dev/google.golang.org/grpc/?tab=doc#ClientConn.NewStream.
type AssetsClient interface {
	FileUpload(ctx context.Context, in *FileUploadRequest, opts ...grpc.CallOption) (*FileUploadResponse, error)
}

type assetsClient struct {
	cc grpc.ClientConnInterface
}

func NewAssetsClient(cc grpc.ClientConnInterface) AssetsClient {
	return &assetsClient{cc}
}

func (c *assetsClient) FileUpload(ctx context.Context, in *FileUploadRequest, opts ...grpc.CallOption) (*FileUploadResponse, error) {
	out := new(FileUploadResponse)
	err := c.cc.Invoke(ctx, Assets_FileUpload_FullMethodName, in, out, opts...)
	if err != nil {
		return nil, err
	}
	return out, nil
}

// AssetsServer is the server API for Assets service.
// All implementations must embed UnimplementedAssetsServer
// for forward compatibility
type AssetsServer interface {
	FileUpload(context.Context, *FileUploadRequest) (*FileUploadResponse, error)
	mustEmbedUnimplementedAssetsServer()
}

// UnimplementedAssetsServer must be embedded to have forward compatible implementations.
type UnimplementedAssetsServer struct {
}

func (UnimplementedAssetsServer) FileUpload(context.Context, *FileUploadRequest) (*FileUploadResponse, error) {
	return nil, status.Errorf(codes.Unimplemented, "method FileUpload not implemented")
}
func (UnimplementedAssetsServer) mustEmbedUnimplementedAssetsServer() {}

// UnsafeAssetsServer may be embedded to opt out of forward compatibility for this service.
// Use of this interface is not recommended, as added methods to AssetsServer will
// result in compilation errors.
type UnsafeAssetsServer interface {
	mustEmbedUnimplementedAssetsServer()
}

func RegisterAssetsServer(s grpc.ServiceRegistrar, srv AssetsServer) {
	s.RegisterService(&Assets_ServiceDesc, srv)
}

func _Assets_FileUpload_Handler(srv interface{}, ctx context.Context, dec func(interface{}) error, interceptor grpc.UnaryServerInterceptor) (interface{}, error) {
	in := new(FileUploadRequest)
	if err := dec(in); err != nil {
		return nil, err
	}
	if interceptor == nil {
		return srv.(AssetsServer).FileUpload(ctx, in)
	}
	info := &grpc.UnaryServerInfo{
		Server:     srv,
		FullMethod: Assets_FileUpload_FullMethodName,
	}
	handler := func(ctx context.Context, req interface{}) (interface{}, error) {
		return srv.(AssetsServer).FileUpload(ctx, req.(*FileUploadRequest))
	}
	return interceptor(ctx, in, info, handler)
}

// Assets_ServiceDesc is the grpc.ServiceDesc for Assets service.
// It's only intended for direct use with grpc.RegisterService,
// and not to be introspected or modified (even as a copy)
var Assets_ServiceDesc = grpc.ServiceDesc{
	ServiceName: "content.v1.Assets",
	HandlerType: (*AssetsServer)(nil),
	Methods: []grpc.MethodDesc{
		{
			MethodName: "FileUpload",
			Handler:    _Assets_FileUpload_Handler,
		},
	},
	Streams:  []grpc.StreamDesc{},
	Metadata: "content/v1/assets.proto",
}
