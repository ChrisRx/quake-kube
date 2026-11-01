package content

import (
	"archive/zip"
	"context"
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"fmt"
	"io"
	"io/fs"
	"log"
	"net/http"
	"net/url"
	"os"
	"path"
	"path/filepath"
	"regexp"
	"strings"
)

// Source is where some of a server's content comes from: a pk3 or a zip archive of game
// directories to download, such as a mod's release, or a directory of game directories,
// such as a content image mounted in the pod.
type Source struct {
	URL string `json:"url"`
	// Image is the content image mounted at Path. It only identifies the source.
	Image string `json:"image"`
	// Path is a directory whose files are linked into the content. Its own directories are
	// game directories, e.g. q3ut4/.
	Path string `json:"path"`
	// SHA256 is the expected checksum of the download, hex encoded.
	SHA256 string `json:"sha256"`
	// Dir is where the files go, relative to the assets directory. A pk3 goes in the game's
	// directory by default, and an archive's or Path's files go at the top.
	Dir string `json:"dir"`
	// StripComponents is how many leading directories to remove from paths in an archive.
	StripComponents int `json:"stripComponents"`
}

// gameDir matches the names of game directories, e.g. baseq3 or cpma.
var gameDir = regexp.MustCompile(`^[A-Za-z0-9_-][A-Za-z0-9_.-]*$`)

// ValidGameDir reports whether name can be used as a game directory.
func ValidGameDir(name string) bool {
	return gameDir.MatchString(name)
}

// Fetch downloads sources into dir. game is the server's game directory, where pk3 files go
// unless their source says otherwise. Each source is recorded once it's in place, so a
// fetch that's interrupted skips what it already has when run again.
func Fetch(ctx context.Context, dir, game string, sources []Source) error {
	if game == "" {
		game = "baseq3"
	}
	if !ValidGameDir(game) {
		return fmt.Errorf("invalid game directory %q", game)
	}
	done := filepath.Join(dir, ".fetched")
	if err := os.MkdirAll(done, 0755); err != nil {
		return err
	}
	for _, src := range sources {
		id, err := json.Marshal(src)
		if err != nil {
			return err
		}
		sum := sha256.Sum256(id)
		marker := filepath.Join(done, hex.EncodeToString(sum[:]))
		if _, err := os.Stat(marker); err == nil {
			log.Printf("Already fetched %s\n", src.URL)
			continue
		}
		if err := fetch(ctx, dir, done, game, src); err != nil {
			return fmt.Errorf("fetching %s: %w", src.URL, err)
		}
		if err := os.WriteFile(marker, id, 0644); err != nil {
			return err
		}
	}
	return nil
}

func fetch(ctx context.Context, dir, tmpDir, game string, src Source) error {
	if src.Path != "" {
		if !validDir(src.Dir) {
			return fmt.Errorf("invalid directory %q", src.Dir)
		}
		return link(src.Path, filepath.Join(dir, filepath.FromSlash(src.Dir)))
	}
	u, err := url.Parse(src.URL)
	if err != nil {
		return err
	}
	if u.Scheme != "http" && u.Scheme != "https" {
		return fmt.Errorf("unsupported URL scheme %q", u.Scheme)
	}
	isPak := strings.EqualFold(path.Ext(u.Path), ".pk3")
	dest := src.Dir
	if dest == "" && isPak {
		dest = game
	}
	if !validDir(dest) {
		return fmt.Errorf("invalid directory %q", src.Dir)
	}

	f, err := os.CreateTemp(tmpDir, "download-*")
	if err != nil {
		return err
	}
	defer os.Remove(f.Name())
	defer f.Close()
	if err := download(ctx, f, src); err != nil {
		return err
	}

	if isPak {
		name := path.Base(u.Path)
		if !ValidGameDir(name) {
			return fmt.Errorf("invalid file name %q", name)
		}
		if err := os.MkdirAll(filepath.Join(dir, dest), 0755); err != nil {
			return err
		}
		return os.Rename(f.Name(), filepath.Join(dir, dest, name))
	}
	return extractZip(f, filepath.Join(dir, filepath.FromSlash(dest)), dest != "", src.StripComponents)
}

// validDir reports whether dir is empty or a relative path of game directory names.
func validDir(dir string) bool {
	if dir == "" {
		return true
	}
	for _, elem := range strings.Split(dir, "/") {
		if !ValidGameDir(elem) {
			return false
		}
	}
	return true
}

// link symlinks the regular files under src into dest, keeping their paths. Files are
// linked rather than copied, since a content image can be gigabytes and is shared by every
// server on the node.
func link(src, dest string) error {
	n := 0
	err := filepath.WalkDir(src, func(path string, d fs.DirEntry, err error) error {
		if err != nil || !d.Type().IsRegular() {
			return err
		}
		rel, err := filepath.Rel(src, path)
		if err != nil {
			return err
		}
		target := filepath.Join(dest, rel)
		if err := os.MkdirAll(filepath.Dir(target), 0755); err != nil {
			return err
		}
		if err := os.Remove(target); err != nil && !os.IsNotExist(err) {
			return err
		}
		n++
		return os.Symlink(path, target)
	})
	if err != nil {
		return err
	}
	log.Printf("Linked %d files from %s\n", n, src)
	return nil
}

func download(ctx context.Context, f *os.File, src Source) error {
	req, err := http.NewRequestWithContext(ctx, http.MethodGet, src.URL, nil)
	if err != nil {
		return err
	}
	resp, err := http.DefaultClient.Do(req)
	if err != nil {
		return err
	}
	defer resp.Body.Close()
	if resp.StatusCode != http.StatusOK {
		return fmt.Errorf("unexpected status %s", resp.Status)
	}
	log.Printf("Downloading %s (%d MB)\n", src.URL, resp.ContentLength>>20)

	h := sha256.New()
	n, err := io.Copy(io.MultiWriter(f, h), resp.Body)
	if err != nil {
		return err
	}
	if got := hex.EncodeToString(h.Sum(nil)); src.SHA256 != "" && !strings.EqualFold(got, src.SHA256) {
		return fmt.Errorf("sha256 is %s, expected %s", got, src.SHA256)
	}
	log.Printf("Downloaded %s (%d MB)\n", src.URL, n>>20)
	return nil
}

// extractZip extracts the archive into dir. Game files live in game directories, so unless
// the archive is extracted into a directory of its own (inDir), files at its top level are
// skipped, e.g. the executables in a standalone game's release.
func extractZip(f *os.File, dir string, inDir bool, strip int) error {
	info, err := f.Stat()
	if err != nil {
		return err
	}
	r, err := zip.NewReader(f, info.Size())
	if err != nil {
		return fmt.Errorf("not a pk3 or zip archive: %w", err)
	}
	for _, zf := range r.File {
		if zf.FileInfo().IsDir() || !zf.Mode().IsRegular() {
			continue
		}
		elems := strings.Split(strings.ReplaceAll(zf.Name, `\`, "/"), "/")
		if len(elems) <= strip {
			continue
		}
		elems = elems[strip:]
		if len(elems) == 1 && !inDir {
			continue
		}
		for _, elem := range elems {
			if elem == "" || elem == "." || elem == ".." {
				return fmt.Errorf("invalid path %q in archive", zf.Name)
			}
		}
		path := filepath.Join(dir, filepath.Join(elems...))
		if err := extractFile(zf, path); err != nil {
			return err
		}
		log.Printf("Extracted %s\n", path)
	}
	return nil
}

func extractFile(zf *zip.File, path string) error {
	if err := os.MkdirAll(filepath.Dir(path), 0755); err != nil {
		return err
	}
	rc, err := zf.Open()
	if err != nil {
		return err
	}
	defer rc.Close()
	out, err := os.Create(path)
	if err != nil {
		return err
	}
	if _, err := io.Copy(out, rc); err != nil {
		out.Close()
		return err
	}
	return out.Close()
}
