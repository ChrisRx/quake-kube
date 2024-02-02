package upload

import (
	"context"
	"crypto/tls"
	"crypto/x509"
	"fmt"
	"os"

	"github.com/spf13/cobra"
	"google.golang.org/grpc"
	"google.golang.org/grpc/credentials"
	"google.golang.org/grpc/credentials/insecure"
	"google.golang.org/protobuf/types/known/emptypb"

	contentapiv1 "github.com/ChrisRx/quake-kube/internal/quake/content/api/v1"
	contentapiv2 "github.com/ChrisRx/quake-kube/internal/quake/content/api/v2"
)

var opts struct {
	Addr     string
	Insecure bool

	// gRPC TLS client auth
	KeyFile    string
	CertFile   string
	CACertFile string
}

func NewCommand() *cobra.Command {
	cmd := &cobra.Command{
		Use:   "upload",
		Short: "upload Quake 3 maps",
		Args:  cobra.ExactArgs(1),
		RunE: func(cmd *cobra.Command, args []string) error {
			var copts []grpc.DialOption
			if opts.Insecure {
				copts = append(copts, grpc.WithTransportCredentials(insecure.NewCredentials()))
			}
			if opts.KeyFile != "" && opts.CertFile != "" {
				tlsConfig := &tls.Config{
					ClientCAs: x509.NewCertPool(),
				}
				if opts.CACertFile != "" {
					caCert, err := os.ReadFile(opts.CACertFile)
					if err != nil {
						return err
					}
					if !tlsConfig.ClientCAs.AppendCertsFromPEM(caCert) {
						return fmt.Errorf("failed to add ca cert file: %s", opts.CACertFile)
					}
				}
				cert, err := tls.LoadX509KeyPair(opts.CertFile, opts.KeyFile)
				if err != nil {
					return err
				}
				tlsConfig.Certificates = []tls.Certificate{cert}
				copts = append(copts, grpc.WithTransportCredentials(credentials.NewTLS(tlsConfig)))
			}
			conn, err := grpc.Dial(opts.Addr, copts...)
			if err != nil {
				return err
			}
			client := contentapiv1.NewAssetsClient(conn)

			data, err := os.ReadFile(args[0])
			if err != nil {
				return err
			}

			resp, err := client.FileUpload(context.Background(), &contentapiv1.FileUploadRequest{
				Name: args[0],
				File: data,
			})
			if err != nil {
				return err
			}
			fmt.Printf("resp: %+v\n", resp)

			{
				client := contentapiv2.NewAssetsClient(conn)
				resp, err := client.GetManifest(context.Background(), &emptypb.Empty{})
				if err != nil {
					return err
				}
				fmt.Printf("resp: %+v\n", resp)
			}

			return nil
		},
	}
	cmd.Flags().StringVar(&opts.Addr, "addr", ":9090", "Address for content server")
	cmd.Flags().BoolVar(&opts.Insecure, "insecure", false, "Allow insecure gRPC client connection")
	return cmd
}
