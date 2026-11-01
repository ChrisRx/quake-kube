package content

import (
	"archive/zip"
	"bytes"
	"io"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/google/go-cmp/cmp"
)

func writePak(t *testing.T, path string, names ...string) {
	t.Helper()
	if err := os.MkdirAll(filepath.Dir(path), 0755); err != nil {
		t.Fatal(err)
	}
	f, err := os.Create(path)
	if err != nil {
		t.Fatal(err)
	}
	defer f.Close()
	zw := zip.NewWriter(f)
	for _, name := range names {
		if _, err := zw.Create(name); err != nil {
			t.Fatal(err)
		}
	}
	if err := zw.Close(); err != nil {
		t.Fatal(err)
	}
}

func TestManifestMapPaks(t *testing.T) {
	dir := t.TempDir()
	writePak(t, filepath.Join(dir, "cpma/map_cpm22.pk3"), "maps/cpm22.bsp", "textures/cpm22/floor.jpg")
	writePak(t, filepath.Join(dir, "cpma/map_cpm26_cpmctf4.pk3"), "maps/cpm26.bsp", "maps/CPMCTF4.bsp")
	writePak(t, filepath.Join(dir, "cpma/z-cpma-pak153.pk3"), "vm/cgame.qvm", "maps/cpm1.aas")
	// Bundles maps under another name, alongside shared content.
	writePak(t, filepath.Join(dir, "q3ut4/zUrT43_004.pk3"), "maps/ut4_turnpike.bsp", "models/players/athena.md3")

	files, err := ReadManifest(dir)
	if err != nil {
		t.Fatal(err)
	}
	maps := map[string][]string{}
	for _, f := range files {
		maps[f.Name] = f.Maps
	}
	expected := map[string][]string{
		"cpma/map_cpm22.pk3":         {"cpm22"},
		"cpma/map_cpm26_cpmctf4.pk3": {"cpm26", "cpmctf4"},
		"cpma/z-cpma-pak153.pk3":     nil,
		"q3ut4/zUrT43_004.pk3":       nil,
	}
	if diff := cmp.Diff(expected, maps); diff != "" {
		t.Errorf("unexpected maps (-want +got):\n%s", diff)
	}
}

func TestPakIndex(t *testing.T) {
	path := filepath.Join(t.TempDir(), "map_test.pk3")
	f, err := os.Create(path)
	if err != nil {
		t.Fatal(err)
	}
	zw := zip.NewWriter(f)
	contents := map[string]string{
		"maps/test.bsp":     strings.Repeat("bsp", 1000),
		"textures/wall.tga": "wall",
	}
	for _, name := range []string{"maps/test.bsp", "textures/wall.tga"} {
		w, err := zw.Create(name)
		if err != nil {
			t.Fatal(err)
		}
		if _, err := w.Write([]byte(contents[name])); err != nil {
			t.Fatal(err)
		}
	}
	if err := zw.Close(); err != nil {
		t.Fatal(err)
	}
	f.Close()

	index, err := ReadPakIndex(path)
	if err != nil {
		t.Fatal(err)
	}
	data, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	if index.Size != int64(len(data)) || index.Directory[1] != index.Size {
		t.Fatalf("unexpected size or directory: %+v", index)
	}

	// A pak made of only the ranges in the index, with everything else zeroed, reads the same
	// as the original.
	var ranges [][2]int64
	for _, r := range index.Files {
		ranges = append(ranges, r)
	}
	ranges = append(ranges, index.Directory)
	r, size, err := OpenRanges(path, ranges)
	if err != nil {
		t.Fatal(err)
	}
	fetched, err := io.ReadAll(r)
	r.Close()
	if err != nil || int64(len(fetched)) != size {
		t.Fatalf("read %d of %d bytes: %v", len(fetched), size, err)
	}
	sparse := make([]byte, len(data))
	for _, rg := range ranges {
		n := copy(sparse[rg[0]:rg[1]], fetched)
		fetched = fetched[n:]
	}
	zr, err := zip.NewReader(bytes.NewReader(sparse), int64(len(sparse)))
	if err != nil {
		t.Fatal(err)
	}
	for _, zf := range zr.File {
		rc, err := zf.Open()
		if err != nil {
			t.Fatal(err)
		}
		got, err := io.ReadAll(rc)
		rc.Close()
		if err != nil {
			t.Fatalf("%s: %v", zf.Name, err)
		}
		if diff := cmp.Diff(contents[zf.Name], string(got)); diff != "" {
			t.Errorf("%s (-want +got):\n%s", zf.Name, diff)
		}
	}

	if _, _, err := OpenRanges(path, [][2]int64{{0, index.Size + 1}}); err == nil {
		t.Error("expected a range past the end to be rejected")
	}
}

func TestReadProfiles(t *testing.T) {
	dir := t.TempDir()
	write := func(game, data string) {
		if err := os.MkdirAll(filepath.Join(dir, game), 0755); err != nil {
			t.Fatal(err)
		}
		if err := os.WriteFile(filepath.Join(dir, game, ProfileName), []byte(data), 0644); err != nil {
			t.Fatal(err)
		}
	}
	write("q3ut4", `{"common": {"q3ut4/a.pk3": ["x.tga"]}, "maps": {"ut4_casa": {"q3ut4/b.pk3": ["casa.bsp"]}}}`)
	write("maps", `{"common": {"q3ut4/a.pk3": ["y.tga"]}, "maps": {"ut4_casa": {"maps/c.pk3": ["casa.jpg"]}}}`)
	// A map pack's, in the mod's directory.
	if err := os.WriteFile(filepath.Join(dir, "q3ut4", "quake-kube-profile-pack.json"),
		[]byte(`{"maps": {"ut4_orbital": {"q3ut4/orbital.pk3": ["orbital.bsp"]}}}`), 0644); err != nil {
		t.Fatal(err)
	}
	if err := os.MkdirAll(filepath.Join(dir, "baseq3"), 0755); err != nil {
		t.Fatal(err)
	}
	// Files next to the games, like the demo's installer, are skipped.
	if err := os.WriteFile(filepath.Join(dir, "linuxq3ademo-1.11-6.x86.gz.sh"), nil, 0644); err != nil {
		t.Fatal(err)
	}

	profile, err := ReadProfiles(dir)
	if err != nil {
		t.Fatal(err)
	}
	expected := &Profile{
		Common: map[string][]string{"q3ut4/a.pk3": {"y.tga", "x.tga"}},
		Maps: map[string]map[string][]string{
			"ut4_casa":    {"q3ut4/b.pk3": {"casa.bsp"}, "maps/c.pk3": {"casa.jpg"}},
			"ut4_orbital": {"q3ut4/orbital.pk3": {"orbital.bsp"}},
		},
	}
	if diff := cmp.Diff(expected, profile); diff != "" {
		t.Errorf("unexpected profile (-want +got):\n%s", diff)
	}
}
