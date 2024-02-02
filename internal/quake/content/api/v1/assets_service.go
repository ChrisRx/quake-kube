package v1

import (
	"archive/zip"
	"bytes"
	"context"
	"fmt"
	"io"
	"os"
	"path/filepath"
	"strings"

	fsutil "github.com/ChrisRx/quake-kube/internal/util/fs"
)

type AssetsService struct {
	UnimplementedAssetsServer

	dir string
}

func NewAssetsService(assetsDir string) *AssetsService {
	return &AssetsService{dir: assetsDir}
}

func (s *AssetsService) FileUpload(ctx context.Context, req *FileUploadRequest) (*FileUploadResponse, error) {
	gameName := "baseq3"
	if fsutil.HasExts(req.Name, ".zip") {
		zr, err := zip.NewReader(bytes.NewReader(req.File), int64(len(req.File)))
		if err != nil {
			return nil, err
		}
		files := make([]string, 0)
		for _, f := range zr.File {
			if !fsutil.HasExts(f.Name, ".pk3") {
				continue
			}
			pak, err := f.Open()
			if err != nil {
				return nil, err
			}
			defer pak.Close()

			dst, err := os.Create(filepath.Join(s.dir, gameName, filepath.Base(f.Name)))
			if err != nil {
				return nil, err
			}
			defer dst.Close()

			if _, err = io.Copy(dst, pak); err != nil {
				return nil, err
			}
			files = append(files, filepath.Base(f.Name))
		}
		if len(files) == 0 {
			return nil, fmt.Errorf("File %s did not contain any map pack files.", req.Name)
		}
		return &FileUploadResponse{
			Name:    req.Name,
			Size:    uint32(len(req.File)),
			Message: fmt.Sprintf("Loaded the following map packs from file %s:\n%s", req.Name, strings.Join(files, "\n")),
		}, nil
	}
	dst, err := os.Create(filepath.Join(s.dir, gameName, req.Name))
	if err != nil {
		return nil, err
	}
	defer dst.Close()

	if _, err = io.Copy(dst, bytes.NewReader(req.File)); err != nil {
		return nil, err
	}
	return &FileUploadResponse{
		Name:    req.Name,
		Size:    uint32(len(req.File)),
		Message: fmt.Sprintf("File %s uploaded successfully.", filepath.Join(gameName, req.Name)),
	}, nil
}
