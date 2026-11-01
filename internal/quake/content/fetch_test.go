package content

import (
	"archive/zip"
	"bytes"
	"context"
	"crypto/sha256"
	"encoding/hex"
	"net/http"
	"net/http/httptest"
	"os"
	"path/filepath"
	"testing"
)

func zipOf(t *testing.T, files map[string]string) []byte {
	var b bytes.Buffer
	w := zip.NewWriter(&b)
	for name, data := range files {
		f, err := w.Create(name)
		if err != nil {
			t.Fatal(err)
		}
		f.Write([]byte(data))
	}
	if err := w.Close(); err != nil {
		t.Fatal(err)
	}
	return b.Bytes()
}

func TestFetch(t *testing.T) {
	release := zipOf(t, map[string]string{
		"Game43/game.exe":         "exe",
		"Game43/q3ut4/zpak.pk3":   "pak",
		"Game43/q3ut4/readme.txt": "readme",
	})
	maps := zipOf(t, map[string]string{"map_cpm22.pk3": "map"})
	evil := zipOf(t, map[string]string{"cpma/../../escaped.pk3": "evil"})
	requests := 0
	srv := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		requests++
		switch r.URL.Path {
		case "/release.zip":
			w.Write(release)
		case "/maps.zip":
			w.Write(maps)
		case "/evil.zip":
			w.Write(evil)
		case "/files/extra.pk3":
			w.Write([]byte("extra"))
		default:
			http.NotFound(w, r)
		}
	}))
	defer srv.Close()

	dir := t.TempDir()
	sum := sha256.Sum256([]byte("extra"))
	sources := []Source{
		{URL: srv.URL + "/release.zip", StripComponents: 1},
		{URL: srv.URL + "/maps.zip", Dir: "cpma"},
		{URL: srv.URL + "/files/extra.pk3", SHA256: hex.EncodeToString(sum[:])},
	}
	if err := Fetch(context.Background(), dir, "q3ut4", sources); err != nil {
		t.Fatal(err)
	}
	for path, want := range map[string]string{
		"q3ut4/zpak.pk3":     "pak",
		"q3ut4/readme.txt":   "readme",
		"cpma/map_cpm22.pk3": "map",
		"q3ut4/extra.pk3":    "extra",
	} {
		got, err := os.ReadFile(filepath.Join(dir, path))
		if err != nil || string(got) != want {
			t.Errorf("%s = %q, %v; want %q", path, got, err, want)
		}
	}
	if _, err := os.Stat(filepath.Join(dir, "game.exe")); !os.IsNotExist(err) {
		t.Errorf("top-level file was extracted: %v", err)
	}

	// Fetching again skips what's already there.
	requests = 0
	if err := Fetch(context.Background(), dir, "q3ut4", sources); err != nil {
		t.Fatal(err)
	}
	if requests != 0 {
		t.Errorf("fetched again: %d requests", requests)
	}

	for _, src := range []Source{
		{URL: srv.URL + "/evil.zip"},
		{URL: srv.URL + "/files/extra.pk3", SHA256: "00"},
		{URL: srv.URL + "/files/extra.pk3", Dir: "../outside"},
		{URL: srv.URL + "/missing.pk3"},
		{URL: "file:///etc/passwd"},
	} {
		if err := Fetch(context.Background(), t.TempDir(), "", []Source{src}); err == nil {
			t.Errorf("expected an error fetching %+v", src)
		}
	}
}

func TestFetchLinksPath(t *testing.T) {
	image := t.TempDir()
	if err := os.MkdirAll(filepath.Join(image, "q3ut4"), 0755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(image, "q3ut4", "zpak.pk3"), []byte("pak"), 0644); err != nil {
		t.Fatal(err)
	}
	dir := t.TempDir()
	src := Source{Image: "example.com/urt:4.3", Path: image}
	if err := Fetch(context.Background(), dir, "q3ut4", []Source{src}); err != nil {
		t.Fatal(err)
	}
	files, err := ReadManifest(dir)
	if err != nil {
		t.Fatal(err)
	}
	if len(files) != 1 || files[0].Name != "q3ut4/zpak.pk3" || files[0].Compressed != 3 {
		t.Fatalf("unexpected manifest: %+v", files)
	}
}
