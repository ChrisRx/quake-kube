package content

import (
	"archive/tar"
	"bytes"
	"compress/gzip"
	"encoding/json"
	"fmt"
	"hash/crc32"
	"io"
	"net/url"
	"os"
	"path/filepath"
	"strings"

	fsutil "github.com/ChrisRx/quake-kube/internal/util/fs"
	httputil "github.com/ChrisRx/quake-kube/internal/util/net/http"
)

// File represents a file on a Quake 3 content server.
type File struct {
	Name       string `json:"name"`
	Compressed int64  `json:"compressed"`
	Checksum   uint32 `json:"checksum"`
}

// DownloadManifest
func DownloadManifest(url string) ([]*File, error) {
	data, err := httputil.GetBody(url + "/assets/manifest.json")
	if err != nil {
		return nil, err
	}

	files := make([]*File, 0)
	if err := json.Unmarshal(data, &files); err != nil {
		return nil, fmt.Errorf("%w: cannot unmarshal %s/assets/manifest.json", err, url)
	}
	return files, nil
}

// ReadManifest
func ReadManifest(dir string) (files []*File, err error) {
	err = fsutil.WalkFiles(dir, func(path string, info os.FileInfo, err error) error {
		data, err := os.ReadFile(path)
		if err != nil {
			return err
		}
		n := crc32.ChecksumIEEE(data)
		path = strings.TrimPrefix(path, dir+"/")
		files = append(files, &File{path, info.Size(), n})
		return nil
	}, ".pk3", ".sh", ".run")
	return
}

// DownloadAssets
func DownloadAssets(u *url.URL, dir string) error {
	url := strings.TrimSuffix(u.String(), "/")
	files, err := DownloadManifest(url)
	if err != nil {
		return err
	}

	for _, f := range files {
		path := filepath.Join(dir, f.Name)
		if _, err := os.Stat(path); !os.IsNotExist(err) {
			continue
		}
		data, err := httputil.GetBody(url + fmt.Sprintf("/assets/%d-%s", f.Checksum, f.Name))
		if err != nil {
			return err
		}
		if err := os.MkdirAll(filepath.Dir(path), 0755); err != nil {
			return err
		}
		if err := os.WriteFile(path, data, 0644); err != nil {
			return err
		}

		// The demo and point releases are compressed gzip files and contain the
		// base pak files needed to play the Quake 3 Arena demo.
		if strings.HasPrefix(f.Name, "linuxq3ademo") || strings.HasPrefix(f.Name, "linuxq3apoint") {
			if err := extractGzip(path, dir); err != nil {
				return err
			}
		}
	}
	return nil
}

var gzipMagicHeader = []byte{'\x1f', '\x8b'}

// extractGzip
func extractGzip(path, dir string) error {
	data, err := os.ReadFile(path)
	if err != nil {
		return err
	}

	// There may be additional header information, so skip directly to the gzip
	// blob.
	idx := bytes.Index(data, gzipMagicHeader)
	data = data[idx:]
	gr, err := gzip.NewReader(bytes.NewReader(data))
	if err != nil {
		return err
	}
	defer gr.Close()

	data, err = io.ReadAll(gr)
	if err != nil {
		return err
	}
	tr := tar.NewReader(bytes.NewReader(data))
	for {
		hdr, err := tr.Next()
		if err == io.EOF {
			break
		}
		if err != nil {
			return err
		}
		if strings.HasSuffix(hdr.Name, ".pk3") {
			fmt.Printf("Downloaded %s\n", hdr.Name)
			data, err := io.ReadAll(tr)
			if err != nil {
				return err
			}

			// Specifically demoq3/pak0.pk3 must be moved to baseq3 to work properly.
			if strings.HasPrefix(hdr.Name, "demoq3/pak0.pk3") {
				hdr.Name = filepath.Join("baseq3", filepath.Base(hdr.Name))
			}
			path := filepath.Join(dir, hdr.Name)
			if err := os.MkdirAll(filepath.Dir(path), 0755); err != nil {
				return err
			}
			if err := os.WriteFile(path, data, 0644); err != nil {
				return err
			}
		}
	}
	return nil
}
