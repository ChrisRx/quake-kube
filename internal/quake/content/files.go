package content

import (
	"archive/tar"
	"bytes"
	"compress/gzip"
	"fmt"
	"hash/crc32"
	"io"
	"os"
	"path/filepath"
	"strings"
	"sync"
	"time"
)

// File represents a file on a Quake 3 content server.
type File struct {
	Name       string `json:"name"`
	Compressed int64  `json:"compressed"`
	Checksum   uint32 `json:"checksum"`
}

// AssetExts are the extensions of the files in a manifest, which are the only files a
// content server serves.
var AssetExts = []string{".pk3", ".sh", ".run"}

type checksumKey struct {
	path    string
	size    int64
	modTime time.Time
}

// checksums caches file checksums, since a mod's content can run to gigabytes.
var checksums sync.Map

// ReadManifest
func ReadManifest(dir string) (files []*File, err error) {
	// walkFiles returns walk errors itself, so the callback never gets one.
	err = walkFiles(dir, func(path string, info os.FileInfo, _ error) error {
		// Content from images is linked in.
		if info.Mode()&os.ModeSymlink != 0 {
			var err error
			if info, err = os.Stat(path); err != nil {
				return err
			}
		}
		key := checksumKey{path, info.Size(), info.ModTime()}
		n, ok := checksums.Load(key)
		if !ok {
			f, err := os.Open(path)
			if err != nil {
				return err
			}
			defer f.Close()
			h := crc32.NewIEEE()
			if _, err := io.Copy(h, f); err != nil {
				return err
			}
			n, _ = checksums.LoadOrStore(key, h.Sum32())
		}
		path = strings.TrimPrefix(path, dir+"/")
		files = append(files, &File{path, info.Size(), n.(uint32)})
		return nil
	}, AssetExts...)
	return
}

var gzipMagicHeader = []byte{'\x1f', '\x8b'}

// ExtractGzip
func ExtractGzip(path, dir string) error {
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
		if strings.HasSuffix(hdr.Name, ".pk3") || strings.HasSuffix(hdr.Name, ".txt") || hdr.Name == "README" {
			fmt.Printf("Extracted %s\n", hdr.Name)
			data, err := io.ReadAll(tr)
			if err != nil {
				return err
			}

			// Specifically demoq3/pak0.pk3 must be moved to baseq3 to work properly.
			if strings.HasPrefix(hdr.Name, "demoq3/pak0.pk3") {
				hdr.Name = filepath.Join("baseq3", filepath.Base(hdr.Name))
			}
			path := filepath.Join(dir, hdr.Name)
			// Keep files that are already there, such as the full game's pak0.pk3 from a
			// server's content.
			if _, err := os.Stat(path); err == nil {
				continue
			}
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
