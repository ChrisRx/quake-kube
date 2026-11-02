package content

import (
	"archive/zip"
	"io"
	"os"
	"path"
	"path/filepath"
	"sort"
	"strings"
)

// Arena is what a map's arena file says about it, e.g. scripts/q3dm17.arena.
type Arena struct {
	// Title is the map's long name, e.g. "The Longest Yard".
	Title string `json:"title,omitempty"`
	// Types are the game types it's made for, e.g. ffa, tourney, team or ctf.
	Types []string `json:"types,omitempty"`
}

// ReadArenas reads the arena files in the content's paks, scripts/*.arena and
// scripts/arenas.txt, by lowercase map name. Later paks take precedence, as in the game.
func ReadArenas(dir string) (map[string]Arena, error) {
	var paks []string
	if err := walkFiles(dir, func(path string, _ os.FileInfo, _ error) error {
		paks = append(paks, path)
		return nil
	}, ".pk3"); err != nil {
		return nil, err
	}
	base := filepath.Join(dir, "baseq3") + string(filepath.Separator)
	sort.Slice(paks, func(i, j int) bool {
		bi, bj := strings.HasPrefix(paks[i], base), strings.HasPrefix(paks[j], base)
		if bi != bj {
			return bi
		}
		return paks[i] < paks[j]
	})
	arenas := make(map[string]Arena)
	for _, pak := range paks {
		r, err := zip.OpenReader(pak)
		if err != nil {
			continue
		}
		for _, f := range r.File {
			name := strings.ToLower(f.Name)
			if path.Dir(name) != "scripts" || (path.Ext(name) != ".arena" && path.Base(name) != "arenas.txt") {
				continue
			}
			rc, err := f.Open()
			if err != nil {
				continue
			}
			data, err := io.ReadAll(io.LimitReader(rc, 1<<20))
			rc.Close()
			if err != nil {
				continue
			}
			for m, arena := range parseArenas(string(data)) {
				arenas[m] = arena
			}
		}
		r.Close()
	}
	return arenas, nil
}

// parseArenas parses blocks of keys and values, e.g.
// { map "q3dm17" longname "The Longest Yard" type "ffa tourney" }. Values may be quoted.
func parseArenas(s string) map[string]Arena {
	arenas := make(map[string]Arena)
	tokens := tokenize(s)
	for i := 0; i < len(tokens); i++ {
		if tokens[i] != "{" {
			continue
		}
		fields := make(map[string]string)
		for i++; i < len(tokens) && tokens[i] != "}"; i += 2 {
			if i+1 < len(tokens) && tokens[i+1] != "}" {
				fields[strings.ToLower(tokens[i])] = tokens[i+1]
			}
		}
		m := strings.ToLower(fields["map"])
		if m == "" {
			continue
		}
		arenas[m] = Arena{
			Title: strings.TrimSpace(fields["longname"]),
			Types: strings.Fields(strings.ToLower(fields["type"])),
		}
	}
	return arenas
}

// tokenize splits an arena file into braces, quoted strings and words, skipping // comments.
func tokenize(s string) []string {
	var tokens []string
	for i := 0; i < len(s); {
		c := s[i]
		switch {
		case c == '/' && i+1 < len(s) && s[i+1] == '/':
			for i < len(s) && s[i] != '\n' {
				i++
			}
		case c == '{' || c == '}':
			tokens = append(tokens, string(c))
			i++
		case c == '"':
			j := strings.IndexByte(s[i+1:], '"')
			if j < 0 {
				j = len(s) - i - 1
			}
			tokens = append(tokens, s[i+1:i+1+j])
			i += j + 2
		case c <= ' ':
			i++
		default:
			j := i
			for j < len(s) && s[j] > ' ' && s[j] != '{' && s[j] != '}' && s[j] != '"' {
				j++
			}
			tokens = append(tokens, s[i:j])
			i = j
		}
	}
	return tokens
}
