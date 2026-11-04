// play-site builds the preview on the docs site: the web app, with static/preview.js in place
// of a gateway, and the games' content laid out for static hosting. Servers are records in the
// browser, and playing one starts a local game with bots in the page.
//
// Paks and the web client are linked rather than copied, since they're large, so copy the
// output with links followed to publish it. A game given with --remote has its files hosted
// elsewhere, which hack/play-upload.sh puts there, since they don't all fit on GitHub Pages.
package main

import (
	"archive/zip"
	"encoding/json"
	"errors"
	"flag"
	"fmt"
	"io/fs"
	"log/slog"
	"maps"
	"os"
	"path"
	"path/filepath"
	"slices"
	"strings"

	"sigs.k8s.io/yaml"

	"github.com/ChrisRx/quake-kube/internal/quake/content"
)

// The thumbnails' size, like the gateway's.
const thumbWidth, thumbHeight = 256, 192

// The namespace preview.js gives its servers.
const namespace = "browser"

// offering is how the preview runs one of the gateway's presets. None use Quake 3's files, which
// the gateway's do, so each has a base game of its own.
type offering struct {
	Preset   string
	BaseGame string
	// Game is the mod it runs on the base game.
	Game  string
	Cvars map[string]string
	// Commands run once the game has started.
	Commands []string
	// OnDemand means it's too large to download whole, so it needs a browser that reads paks in
	// parts.
	OnDemand bool
}

var offerings = []offering{
	{Preset: "openarena", BaseGame: "baseoa"},
	{Preset: "openarena-ctf", BaseGame: "baseoa"},
	// It leaves players spectating, whatever g_teamAutoJoin says, until they ask to join.
	{Preset: "cpma", BaseGame: "baseoa", Game: "cpma", Commands: []string{"wait 600", "team r"}},
	// Its bots are off unless they're enabled.
	{Preset: "urban-terror", BaseGame: "q3ut4", Cvars: map[string]string{"bot_enable": "1"}, OnDemand: true},
}

func (o offering) contentGame() string {
	if o.Game != "" {
		return o.Game
	}
	return o.BaseGame
}

type options struct {
	Static        string
	Client        string
	ClientVersion string
	// Content has directories with game directories in them.
	Content []string
	// Remote has the URLs game directories are hosted at, by game.
	Remote  map[string]string
	Presets string
	Out     string
}

func main() {
	opts := options{Remote: map[string]string{}}
	flag.StringVar(&opts.Static, "static", "controllers/static", "the web app's files")
	flag.StringVar(&opts.Client, "client", "", "the web client's build")
	flag.StringVar(&opts.ClientVersion, "client-version", "preview", "changes whenever the web client does")
	flag.Func("content", "a directory with game directories, which can be repeated", func(dir string) error {
		opts.Content = append(opts.Content, dir)
		return nil
	})
	flag.Func("remote", "<game>=<URL> for a game whose files are hosted elsewhere, which can be repeated", func(s string) error {
		game, url, ok := strings.Cut(s, "=")
		if !ok {
			return errors.New("expected <game>=<URL>")
		}
		opts.Remote[game] = strings.TrimSuffix(url, "/")
		return nil
	})
	flag.StringVar(&opts.Presets, "presets", "hack/k3d/presets.yaml", "the file with the presets to offer")
	flag.StringVar(&opts.Out, "out", "", "the directory to write the site to")
	dir := flag.String("serve", "", "serve this directory instead of building, to try a built site")
	addr := flag.String("addr", "127.0.0.1:3000", "the address to serve on")
	flag.Parse()
	if *dir != "" {
		if err := serve(*dir, *addr); err != nil {
			fmt.Fprintln(os.Stderr, "play-site:", err)
			os.Exit(1)
		}
		return
	}
	if opts.Client == "" || len(opts.Content) == 0 || opts.Out == "" {
		flag.Usage()
		os.Exit(2)
	}
	if err := build(opts); err != nil {
		fmt.Fprintln(os.Stderr, "play-site:", err)
		os.Exit(1)
	}
}

func build(opts options) error {
	for _, dir := range []string{"static", "client", "content", "paks"} {
		if err := os.MkdirAll(filepath.Join(opts.Out, dir), 0o755); err != nil {
			return err
		}
	}
	if err := writePages(opts); err != nil {
		return err
	}
	if err := linkClient(opts); err != nil {
		return err
	}
	roots, err := gameRoots(opts.Content)
	if err != nil {
		return err
	}
	presets, err := writePresets(opts, roots)
	if err != nil {
		return err
	}
	games := map[string]bool{}
	for _, p := range presets {
		games[p.BaseGame] = true
		if p.Game != "" {
			games[p.Game] = true
		}
	}
	files, err := writeContent(opts, roots, games)
	if err != nil {
		return err
	}
	return writeScript(opts, presets, files)
}

// gameRoots finds the game directories in the content, returning the directory each is in.
func gameRoots(dirs []string) (map[string]string, error) {
	roots := make(map[string]string)
	for _, dir := range dirs {
		dir, err := filepath.Abs(dir)
		if err != nil {
			return nil, err
		}
		entries, err := os.ReadDir(dir)
		if err != nil {
			return nil, err
		}
		for _, entry := range entries {
			if entry.IsDir() {
				roots[entry.Name()] = dir
			}
		}
	}
	return roots, nil
}

// replaceAll fails when a page has changed, rather than leaving one that doesn't work.
func replaceAll(page, from, to string) (string, error) {
	if !strings.Contains(page, from) {
		return "", fmt.Errorf("page has no %q", from)
	}
	return strings.ReplaceAll(page, from, to), nil
}

func writePages(opts options) error {
	const scripts = `<head>
    <script src="static/presets.js"></script>
    <script src="static/preview.js"></script>`
	pages := []struct {
		src, dst     string
		replacements [][2]string
	}{
		{src: "index.html", dst: "index.html"},
		{src: "play.html", dst: "game.html", replacements: [][2]string{
			// There's no game traffic to relay.
			{`<script src="/static/gamesocket.js"></script>`, ""},
			{`"{{NAMESPACE}}"`, `"` + namespace + `"`},
			{`"{{NAME}}"`, "preview.server.name"},
			{`{{NEEDS_PASSWORD}}`, "false"},
			{`{{MAPS}}`, "preview.server.maps"},
			{`{{CLIENT_VERSION}}`, opts.ClientVersion},
			{`{{ON_DEMAND}}`, "true"},
			{`{{WEBTRANSPORT}}`, "null"},
			{`{{SUGGESTED_NAME}}`, "null"},
			{`{{MANAGE}}`, "preview.manage()"},
			// preview.js fills in the server's name, in these two.
			{`<title>{{NAME}} · `, `<title>Preview · `},
			{`<h1>{{NAME}}</h1>`, `<h1></h1>`},
			{`<a href="/">Back to servers</a>`, `<a href="./">Back to servers</a>`},
		}},
	}
	for _, page := range pages {
		data, err := os.ReadFile(filepath.Join(opts.Static, page.src))
		if err != nil {
			return err
		}
		html := string(data)
		replacements := append(page.replacements, [2]string{"<head>", scripts}, [2]string{`src="/static/`, `src="static/`})
		for _, r := range replacements {
			if html, err = replaceAll(html, r[0], r[1]); err != nil {
				return fmt.Errorf("%s: %w", page.src, err)
			}
		}
		if err := os.WriteFile(filepath.Join(opts.Out, page.dst), []byte(html), 0o644); err != nil {
			return err
		}
	}
	for _, name := range []string{"maps.js", "paks.js", "preview.js"} {
		data, err := os.ReadFile(filepath.Join(opts.Static, name))
		if err != nil {
			return err
		}
		if err := os.WriteFile(filepath.Join(opts.Out, "static", name), data, 0o644); err != nil {
			return err
		}
	}
	return nil
}

func linkClient(opts options) error {
	client, err := filepath.Abs(opts.Client)
	if err != nil {
		return err
	}
	for _, dir := range []string{"", "jspi"} {
		if err := os.MkdirAll(filepath.Join(opts.Out, "client", dir), 0o755); err != nil {
			return err
		}
		for _, name := range []string{"ioquake3.js", "ioquake3.wasm"} {
			if err := os.Symlink(filepath.Join(client, dir, name), filepath.Join(opts.Out, "client", dir, name)); err != nil {
				return err
			}
		}
	}
	return nil
}

// writeContent links in the games' paks, with what the play page reads to fetch parts of them:
// the manifest, each pak's index, and the profile of the files each map reads. It returns the
// games' loose files, which the game also reads, like a mod's configs for its modes.
func writeContent(opts options, roots map[string]string, games map[string]bool) ([]string, error) {
	var manifest []*content.File
	loose := []string{}
	merged := &content.Profile{Common: map[string][]string{}, Maps: map[string]map[string][]string{}}
	seen := map[string]bool{}
	for _, game := range slices.Sorted(maps.Keys(games)) {
		root := roots[game]
		if err := os.MkdirAll(filepath.Join(opts.Out, "paks", game), 0o755); err != nil {
			return nil, err
		}
		_, remote := opts.Remote[game]
		if !remote {
			if err := os.MkdirAll(filepath.Join(opts.Out, "content", game), 0o755); err != nil {
				return nil, err
			}
		}
		files, err := content.ReadManifest(root)
		if err != nil {
			return nil, err
		}
		for _, f := range files {
			if !strings.HasPrefix(f.Name, game+"/") {
				continue
			}
			src := filepath.Join(root, f.Name)
			manifest = append(manifest, f)
			if !remote {
				if err := os.Symlink(src, filepath.Join(opts.Out, "content", f.Name)); err != nil {
					return nil, err
				}
			}
			index, err := content.ReadPakIndex(src)
			if err != nil {
				return nil, err
			}
			if err := writeJSON(filepath.Join(opts.Out, "paks", f.Name+".json"), index); err != nil {
				return nil, err
			}
		}
		read, err := looseFiles(root, game, opts.Out, !remote)
		if err != nil {
			return nil, err
		}
		loose = append(loose, read...)
		if !seen[root] {
			seen[root] = true
			profile, err := content.ReadProfiles(root)
			if err != nil {
				return nil, err
			}
			mergeProfile(merged, profile, games)
		}
	}
	if len(manifest) == 0 {
		return nil, errors.New("no paks in the content")
	}
	if err := writeJSON(filepath.Join(opts.Out, "manifest.json"), manifest); err != nil {
		return nil, err
	}
	return loose, writeJSON(filepath.Join(opts.Out, "profile.json"), merged)
}

// looseFiles finds a game's files other than its paks, optionally linking them in, and returns
// those the game reads. The rest are licenses and readmes, which go with the content. A game
// server has these on its disk and doesn't send them to players, but here the page is the server.
func looseFiles(root, game, out string, link bool) ([]string, error) {
	var read []string
	err := filepath.WalkDir(filepath.Join(root, game), func(name string, entry fs.DirEntry, err error) error {
		if err != nil || entry.IsDir() {
			return err
		}
		rel, err := filepath.Rel(root, name)
		if err != nil {
			return err
		}
		if filepath.Ext(rel) == ".pk3" || strings.HasPrefix(filepath.Base(rel), strings.TrimSuffix(content.ProfileName, ".json")) {
			return nil
		}
		if link {
			dst := filepath.Join(out, "content", rel)
			if err := os.MkdirAll(filepath.Dir(dst), 0o755); err != nil {
				return err
			}
			if err := os.Symlink(name, dst); err != nil {
				return err
			}
		}
		// Those in a game's top directory are about it, rather than for it.
		if strings.Contains(strings.TrimPrefix(rel, game+string(filepath.Separator)), string(filepath.Separator)) {
			read = append(read, filepath.ToSlash(rel))
		}
		return nil
	})
	return read, err
}

// mergeProfile adds a profile's files for these games. The profiles were made with Quake 3's
// demo files present, which the preview doesn't have.
func mergeProfile(merged, profile *content.Profile, games map[string]bool) {
	add := func(to, from map[string][]string) {
		for pak, files := range from {
			if games[strings.SplitN(pak, "/", 2)[0]] {
				to[pak] = append(to[pak], files...)
			}
		}
	}
	add(merged.Common, profile.Common)
	for name, files := range profile.Maps {
		if merged.Maps[name] == nil {
			merged.Maps[name] = map[string][]string{}
		}
		add(merged.Maps[name], files)
	}
}

// The parts of a preset the web app uses, in the gateway's JSON, with how the preview runs it.
type preset struct {
	Name      string                   `json:"name"`
	Title     string                   `json:"title"`
	GameTypes []gameType               `json:"gameTypes"`
	Maps      []string                 `json:"maps"`
	Bots      bool                     `json:"bots"`
	Levelshot bool                     `json:"levelshots"`
	MapInfo   map[string]content.Arena `json:"mapInfo"`
	MapLists  []mapList                `json:"mapLists"`
	Spec      struct {
		Cvars map[string]string `json:"cvars,omitempty"`
	} `json:"spec"`

	BaseGame string            `json:"baseGame"`
	Game     string            `json:"game,omitempty"`
	Cvars    map[string]string `json:"cvars,omitempty"`
	Commands []string          `json:"commands,omitempty"`
	OnDemand bool              `json:"onDemand,omitempty"`
}

type gameType struct {
	Name  string            `json:"name"`
	Type  int               `json:"type"`
	Arena string            `json:"arena,omitempty"`
	Cvars map[string]string `json:"cvars,omitempty"`
}

type mapList struct {
	Name string   `json:"name"`
	Maps []string `json:"maps"`
}

// writePresets writes the thumbnails of the offered presets' maps, returning the presets.
func writePresets(opts options, roots map[string]string) ([]preset, error) {
	data, err := os.ReadFile(opts.Presets)
	if err != nil {
		return nil, err
	}
	// Only those offered are read, since other games name their game types rather than number
	// them.
	var file struct {
		Gateway struct {
			Presets []json.RawMessage `json:"presets"`
		} `json:"gateway"`
	}
	if err := yaml.Unmarshal(data, &file); err != nil {
		return nil, err
	}
	raws := make(map[string]json.RawMessage)
	for _, raw := range file.Gateway.Presets {
		var named struct {
			Name string `json:"name"`
		}
		if err := json.Unmarshal(raw, &named); err != nil {
			return nil, err
		}
		raws[named.Name] = raw
	}
	var presets []preset
	for _, o := range offerings {
		root, ok := roots[o.contentGame()]
		raw := raws[o.Preset]
		if !ok || raw == nil {
			slog.Info("not offering a preset without its content", slog.String("preset", o.Preset))
			continue
		}
		if _, ok := roots[o.BaseGame]; !ok {
			return nil, fmt.Errorf("preset %s needs %s", o.Preset, o.BaseGame)
		}
		var p preset
		if err := json.Unmarshal(raw, &p); err != nil {
			return nil, fmt.Errorf("preset %s: %w", o.Preset, err)
		}
		p.BaseGame, p.Game, p.Cvars, p.Commands, p.OnDemand = o.BaseGame, o.Game, o.Cvars, o.Commands, o.OnDemand
		if err := addMaps(&p, root, filepath.Join(root, o.contentGame()), opts.Out); err != nil {
			return nil, fmt.Errorf("preset %s: %w", o.Preset, err)
		}
		presets = append(presets, p)
	}
	if len(presets) == 0 {
		return nil, errors.New("no presets with their content")
	}
	return presets, nil
}

// addMaps keeps the preset's maps the preview can play and writes their thumbnails.
func addMaps(p *preset, root, gameDir, out string) error {
	playable, err := botMaps(gameDir)
	if err != nil {
		return err
	}
	arenas, err := content.ReadArenas(root)
	if err != nil {
		return err
	}
	// The presets can offer maps of Quake 3's, which the preview doesn't have, and bots can't
	// play on maps without navigation files.
	p.Maps = slices.DeleteFunc(p.Maps, func(name string) bool {
		return !playable[strings.ToLower(name)]
	})
	if len(p.Maps) == 0 {
		return errors.New("none of its maps are playable")
	}
	p.Bots, p.Levelshot = true, true
	p.MapInfo = map[string]content.Arena{}
	p.MapLists = []mapList{}
	shots := filepath.Join(out, "api", "presets", p.Name, "levelshots")
	if err := os.MkdirAll(shots, 0o755); err != nil {
		return err
	}
	for _, name := range p.Maps {
		name = strings.ToLower(name)
		if arena, ok := arenas[name]; ok {
			p.MapInfo[name] = arena
		}
		shot, err := content.ReadLevelshot(root, name)
		if errors.Is(err, content.ErrNoLevelshot) {
			slog.Info("map has no levelshot", slog.String("map", name))
			continue
		}
		if err == nil {
			shot, err = content.Thumbnail(shot, thumbWidth, thumbHeight)
		}
		if err != nil {
			return fmt.Errorf("levelshot of %s: %w", name, err)
		}
		if err := os.WriteFile(filepath.Join(shots, name+".jpg"), shot, 0o644); err != nil {
			return err
		}
	}
	return nil
}

// botMaps returns the lowercase names of the maps in a game's paks with navigation files.
func botMaps(dir string) (map[string]bool, error) {
	paks, err := filepath.Glob(filepath.Join(dir, "*.pk3"))
	if err != nil {
		return nil, err
	}
	found := make(map[string]bool)
	for _, pak := range paks {
		r, err := zip.OpenReader(pak)
		if err != nil {
			return nil, fmt.Errorf("%s: %w", pak, err)
		}
		for _, f := range r.File {
			name := strings.ToLower(f.Name)
			if path.Dir(name) == "maps" {
				found[path.Base(name)] = true
			}
		}
		r.Close()
	}
	playable := make(map[string]bool)
	for name := range found {
		if m, ok := strings.CutSuffix(name, ".bsp"); ok && found[m+".aas"] {
			playable[m] = true
		}
	}
	return playable, nil
}

// writeScript writes what preview.js needs before the pages start, so the play page's settings
// have it.
func writeScript(opts options, presets []preset, loose []string) error {
	values := []struct {
		name  string
		value any
	}{
		{"previewPresets", presets},
		{"previewRemote", opts.Remote},
		{"previewLoose", loose},
	}
	var script strings.Builder
	for _, v := range values {
		encoded, err := json.Marshal(v.value)
		if err != nil {
			return err
		}
		fmt.Fprintf(&script, "globalThis.%s = %s;\n", v.name, encoded)
	}
	return os.WriteFile(filepath.Join(opts.Out, "static", "presets.js"), []byte(script.String()), 0o644)
}

func writeJSON(name string, v any) error {
	data, err := json.Marshal(v)
	if err != nil {
		return err
	}
	return os.WriteFile(name, data, 0o644)
}
