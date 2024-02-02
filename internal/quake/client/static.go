package client

import (
	"embed"
	"io"
	"io/fs"
	"net/http"

	fsutil "github.com/ChrisRx/quake-kube/internal/util/fs"
)

//go:embed static
var static embed.FS

type staticFiles struct {
	http.FileSystem
}

func LoadStaticFiles() (*staticFiles, error) {
	files, err := fs.Sub(static, "static")
	if err != nil {
		return nil, err
	}

	// TODO(ChrisRx): I don't remember why I was stripping the modified time out
	// anymore lol
	return &staticFiles{fsutil.StripModTime(http.FS(files))}, nil
}

func (s *staticFiles) ReadFile(path string) ([]byte, error) {
	f, err := s.Open(path)
	if err != nil {
		return nil, err
	}
	defer f.Close()

	return io.ReadAll(f)
}
