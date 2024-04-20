package server

import (
	"archive/tar"
	"bytes"
	_ "embed"
	"io"
	"log"
	"os"
	"path/filepath"
	"strings"

	contentutil "github.com/ChrisRx/quake-kube/internal/quake/content/util"
)

//go:embed EULA.txt
var Q3DemoEULA []byte

//go:embed gamefiles.tar
var gamefiles []byte

func ExtractGameFiles(dir string) error {
	tr := tar.NewReader(bytes.NewReader(gamefiles))

	for {
		hdr, err := tr.Next()
		if err == io.EOF {
			break
		}
		if err != nil {
			return err
		}
		switch hdr.Typeflag {
		case tar.TypeReg:
			path := filepath.Join(dir, hdr.Name)
			if _, err := os.Stat(path); !os.IsNotExist(err) {
				continue
			}
			log.Printf("Extracting: %s\n", path)
			data, err := io.ReadAll(tr)
			if err != nil {
				return err
			}
			if err := os.MkdirAll(filepath.Dir(path), 0755); err != nil {
				return err
			}
			if err := os.WriteFile(path, data, 0644); err != nil {
				return err
			}
			if strings.HasPrefix(hdr.Name, "linuxq3ademo") || strings.HasPrefix(hdr.Name, "linuxq3apoint") {
				if err := contentutil.ExtractGzip(path, dir); err != nil {
					return err
				}
			}
		}
	}
	return nil
}
