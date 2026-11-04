package main

import (
	"archive/zip"
	"bytes"
	"encoding/json"
	"image"
	"image/jpeg"
	"os"
	"path/filepath"
	"slices"
	"strings"
	"testing"

	"github.com/ChrisRx/quake-kube/internal/quake/content"
)

// writePak writes a pak with an OpenArena map, its levelshot and arena file.
func writePak(t *testing.T, path, mapName string) {
	t.Helper()
	var shot bytes.Buffer
	if err := jpeg.Encode(&shot, image.NewRGBA(image.Rect(0, 0, 8, 8)), nil); err != nil {
		t.Fatal(err)
	}
	if err := os.MkdirAll(filepath.Dir(path), 0o755); err != nil {
		t.Fatal(err)
	}
	f, err := os.Create(path)
	if err != nil {
		t.Fatal(err)
	}
	defer f.Close()
	w := zip.NewWriter(f)
	for name, data := range map[string][]byte{
		"maps/" + mapName + ".bsp":       []byte("bsp"),
		"maps/" + mapName + ".aas":       []byte("aas"),
		"maps/no_bots.bsp":               []byte("bsp"),
		"levelshots/" + mapName + ".jpg": shot.Bytes(),
		"scripts/" + mapName + ".arena":  []byte(`{ map "` + mapName + `" longname "Test Map" type "ffa" }`),
	} {
		entry, err := w.Create(name)
		if err != nil {
			t.Fatal(err)
		}
		if _, err := entry.Write(data); err != nil {
			t.Fatal(err)
		}
	}
	if err := w.Close(); err != nil {
		t.Fatal(err)
	}
}

func writeFile(t *testing.T, path, data string) {
	t.Helper()
	if err := os.MkdirAll(filepath.Dir(path), 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(path, []byte(data), 0o644); err != nil {
		t.Fatal(err)
	}
}

func read(t *testing.T, path string) string {
	t.Helper()
	data, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	return string(data)
}

func TestBuild(t *testing.T) {
	dir := t.TempDir()
	game := filepath.Join(dir, "content", "baseoa")
	writeFile(t, filepath.Join(game, "COPYING"), "GPL")
	writeFile(t, filepath.Join(game, content.ProfileName), `{
		"common": {"baseoa/pak0.pk3": ["maps/oa_test.bsp"], "baseq3/pak0.pk3": ["scripts/x.shader"]},
		"maps": {"oa_test": {"baseoa/pak0.pk3": ["levelshots/oa_test.jpg"], "baseq3/pak0.pk3": ["y"]}}
	}`)
	writePak(t, filepath.Join(game, "pak0.pk3"), "oa_test")
	mod := filepath.Join(dir, "mod", "cpma")
	writePak(t, filepath.Join(mod, "pak.pk3"), "cpma_test")
	writeFile(t, filepath.Join(mod, "readme.txt"), "readme")
	writeFile(t, filepath.Join(mod, "modes", "CA.cfg"), "mode")
	urt := filepath.Join(dir, "urt", "q3ut4")
	writePak(t, filepath.Join(urt, "pak.pk3"), "ut_test")
	writeFile(t, filepath.Join(urt, "modes", "x.cfg"), "mode")
	client := filepath.Join(dir, "client")
	for _, name := range []string{"ioquake3.js", "ioquake3.wasm", "jspi/ioquake3.js", "jspi/ioquake3.wasm"} {
		writeFile(t, filepath.Join(client, name), "client")
	}
	presets := filepath.Join(dir, "presets.yaml")
	writeFile(t, presets, `
gateway:
  presets:
    - name: quake3
      title: Quake 3
      gameTypes: [{name: Free For All, type: FreeForAll}]
      maps: [q3dm1]
    - name: openarena
      title: OpenArena
      gameTypes: [{name: Free For All, type: 0, arena: ffa}, {name: Elimination, type: 8}]
      maps: [OA_Test, q3dm17, no_bots]
    - name: openarena-ctf
      title: OpenArena CTF
      gameTypes: [{name: Capture The Flag, type: 4, arena: ctf}]
      maps: [oa_test, ironwood]
    - name: cpma
      title: CPMA
      gameTypes: [{name: Clan Arena, cvars: {mode_start: CA}}]
      maps: [cpma_test]
      spec:
        cvars: {server_useMapModes: "0"}
    - name: urban-terror
      title: Urban Terror
      bots: false
      gameTypes: [{name: Free For All, type: 0}]
      maps: [ut_test]
`)
	out := filepath.Join(dir, "out")
	err := build(options{
		Static:        filepath.Join("..", "..", "controllers", "static"),
		Client:        client,
		ClientVersion: "v1",
		Content:       []string{filepath.Join(dir, "content"), filepath.Join(dir, "mod"), filepath.Join(dir, "urt")},
		Remote:        map[string]string{"q3ut4": "https://content.example"},
		Presets:       presets,
		Out:           out,
	})
	if err != nil {
		t.Fatal(err)
	}

	for _, page := range []string{"index.html", "game.html"} {
		html := read(t, filepath.Join(out, page))
		if strings.Contains(html, "{{") {
			t.Errorf("%s has placeholders left", page)
		}
		if strings.Contains(html, `src="/static/`) {
			t.Errorf("%s has paths that only work on a gateway", page)
		}
		first := strings.Index(html, `src="static/`)
		if first < 0 || !strings.HasPrefix(html[first:], `src="static/presets.js"`) {
			t.Errorf("%s doesn't load the presets first", page)
		}
	}
	if strings.Contains(read(t, filepath.Join(out, "game.html")), "gamesocket") {
		t.Error("game.html loads gamesocket.js")
	}

	var manifest []*content.File
	if err := json.Unmarshal([]byte(read(t, filepath.Join(out, "manifest.json"))), &manifest); err != nil {
		t.Fatal(err)
	}
	names := []string{}
	for _, f := range manifest {
		names = append(names, f.Name)
	}
	if want := []string{"baseoa/pak0.pk3", "cpma/pak.pk3", "q3ut4/pak.pk3"}; !slices.Equal(names, want) {
		t.Errorf("manifest = %v, want %v", names, want)
	}
	if _, err := os.Lstat(filepath.Join(out, "content", "q3ut4", "pak.pk3")); !os.IsNotExist(err) {
		t.Errorf("a remote game's pak is in the site: %v", err)
	}
	if _, err := os.Stat(filepath.Join(out, "paks", "q3ut4", "pak.pk3.json")); err != nil {
		t.Errorf("a remote game's pak has no index: %v", err)
	}
	if _, err := os.Lstat(filepath.Join(out, "content", "q3ut4", "modes", "x.cfg")); !os.IsNotExist(err) {
		t.Errorf("a remote game's loose file is in the site: %v", err)
	}
	if read(t, filepath.Join(out, "content", "cpma", "modes", "CA.cfg")) != "mode" {
		t.Error("a game's loose files aren't with its content")
	}
	if read(t, filepath.Join(out, "content", "baseoa", "COPYING")) != "GPL" {
		t.Error("the license isn't with the content")
	}

	var index content.PakIndex
	if err := json.Unmarshal([]byte(read(t, filepath.Join(out, "paks", "baseoa", "pak0.pk3.json"))), &index); err != nil {
		t.Fatal(err)
	}
	if _, ok := index.Files["maps/oa_test.bsp"]; !ok {
		t.Errorf("index has %v", index.Files)
	}

	if strings.Contains(read(t, filepath.Join(out, "profile.json")), "baseq3") {
		t.Error("profile has Quake 3's paks")
	}

	script := read(t, filepath.Join(out, "static", "presets.js"))
	lines := strings.Split(script, "\n")
	value := func(line, name string, v any) {
		t.Helper()
		encoded := strings.TrimSuffix(strings.TrimPrefix(line, "globalThis."+name+" = "), ";")
		if err := json.Unmarshal([]byte(encoded), v); err != nil {
			t.Fatal(err)
		}
	}
	var got []preset
	value(lines[0], "previewPresets", &got)
	gotNames := []string{}
	for _, p := range got {
		gotNames = append(gotNames, p.Name)
	}
	if want := []string{"openarena", "openarena-ctf", "cpma", "urban-terror"}; !slices.Equal(gotNames, want) {
		t.Fatalf("presets = %v, want %v", gotNames, want)
	}
	if len(got[0].Maps) != 1 || got[0].Maps[0] != "OA_Test" {
		t.Errorf("maps = %v, want only those bots can play", got[0].Maps)
	}
	if got[0].GameTypes[1].Type != 8 {
		t.Errorf("game types = %+v", got[0].GameTypes)
	}
	if got[0].MapInfo["oa_test"].Title != "Test Map" {
		t.Errorf("map info = %+v", got[0].MapInfo)
	}
	cpma, urbanTerror := got[2], got[3]
	if cpma.BaseGame != "baseoa" || cpma.Game != "cpma" || cpma.Spec.Cvars["server_useMapModes"] != "0" || cpma.GameTypes[0].Cvars["mode_start"] != "CA" {
		t.Errorf("cpma = %+v", cpma)
	}
	if urbanTerror.BaseGame != "q3ut4" || !urbanTerror.Bots || urbanTerror.Cvars["bot_enable"] != "1" || !urbanTerror.OnDemand {
		t.Errorf("urban terror = %+v", urbanTerror)
	}
	var remote map[string]string
	value(lines[1], "previewRemote", &remote)
	if remote["q3ut4"] != "https://content.example" {
		t.Errorf("remote = %v", remote)
	}
	var loose []string
	value(lines[2], "previewLoose", &loose)
	if !slices.Equal(loose, []string{"cpma/modes/CA.cfg", "q3ut4/modes/x.cfg"}) {
		t.Errorf("loose files the game reads = %v", loose)
	}
	if _, err := os.Stat(filepath.Join(out, "api", "presets", "openarena", "levelshots", "oa_test.jpg")); err != nil {
		t.Error(err)
	}
}
