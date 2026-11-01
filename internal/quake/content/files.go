package content

import (
	"archive/tar"
	"archive/zip"
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
	// Maps is set for a map's pak, which browser clients only need when playing one of
	// these maps. Other files are always needed.
	Maps []string `json:"maps,omitempty"`
}

// AssetExts are the extensions of the files in a manifest, which are the only files a
// content server serves.
var AssetExts = []string{".pk3", ".sh", ".run"}

type checksumKey struct {
	path    string
	size    int64
	modTime time.Time
}

// scanned caches what's found reading files, since a mod's content can run to gigabytes.
var scanned sync.Map

type scan struct {
	checksum uint32
	maps     []string
}

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
		v, ok := scanned.Load(key)
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
			var maps []string
			if strings.HasSuffix(path, ".pk3") {
				// A pak that can't be read is left to the engine to complain about, and
				// counts as always needed.
				maps, _ = mapPakMaps(f, info.Size(), filepath.Base(path))
			}
			v, _ = scanned.LoadOrStore(key, &scan{h.Sum32(), maps})
		}
		s := v.(*scan)
		path = strings.TrimPrefix(path, dir+"/")
		files = append(files, &File{path, info.Size(), s.checksum, s.maps})
		return nil
	}, AssetExts...)
	return
}

// mapPakMaps returns the maps a pak is for, when it's a map's pak: one whose name includes the
// name of every map in it, like map_cpm22.pk3. Paks that bundle maps under other names, like
// Urban Terror's zUrT43_004.pk3, often carry shared content too, so they're always needed.
func mapPakMaps(r io.ReaderAt, size int64, name string) ([]string, error) {
	zr, err := zip.NewReader(r, size)
	if err != nil {
		return nil, err
	}
	name = strings.ToLower(name)
	var maps []string
	for _, f := range zr.File {
		path := strings.ToLower(f.Name)
		if !strings.HasPrefix(path, "maps/") || !strings.HasSuffix(path, ".bsp") {
			continue
		}
		m := strings.TrimSuffix(strings.TrimPrefix(path, "maps/"), ".bsp")
		if strings.Contains(m, "/") {
			continue
		}
		if !strings.Contains(name, m) {
			return nil, nil
		}
		maps = append(maps, m)
	}
	return maps, nil
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
