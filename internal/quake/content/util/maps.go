package content

import (
	"archive/zip"
	"os"
	"path/filepath"
	"strings"

	fsutil "github.com/ChrisRx/quake-kube/internal/util/fs"
)

type Map struct {
	File string `json:"file"`
	Name string `json:"name"`
}

func ReadMaps(dir string) (result []*Map, err error) {
	err = fsutil.WalkFiles(dir, func(path string, info os.FileInfo, err error) error {
		maps, err := OpenMapPack(path)
		if err != nil {
			return err
		}
		result = append(result, maps...)
		return err
	}, ".pk3")
	return
}

// OpenMapPack
// pk3s = zip files
func OpenMapPack(path string) ([]*Map, error) {
	f, err := os.Open(path)
	if err != nil {
		return nil, err
	}
	defer f.Close()

	info, err := f.Stat()
	if err != nil {
		return nil, err
	}
	r, err := zip.NewReader(f, info.Size())
	if err != nil {
		return nil, err
	}
	maps := make([]*Map, 0)
	for _, f := range r.File {
		if !fsutil.HasExts(f.Name, ".bsp") {
			continue
		}
		path := filepath.Join(filepath.Base(filepath.Dir(path)), filepath.Base(path))
		mapName := strings.TrimSuffix(filepath.Base(f.Name), ".bsp")
		maps = append(maps, &Map{File: path, Name: mapName})
	}
	return maps, nil
}
