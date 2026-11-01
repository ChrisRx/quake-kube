// mdbook-pikchr is an mdBook preprocessor that draws ```pikchr blocks as SVGs when the docs
// are built. Lines and text take the page's color, so diagrams follow the theme, and each
// diagram has the width it was drawn at in --diagram-width, which the theme never draws it
// wider than, and opens it larger from when it's shrunk.
//
// mdBook runs it as `mdbook-pikchr supports <renderer>`, then with the book's JSON on stdin,
// expecting the book back on stdout.
package main

import (
	"encoding/json"
	"fmt"
	"io"
	"os"
	"strings"

	"github.com/gopikchr/gopikchr"
)

func main() {
	if len(os.Args) > 1 && os.Args[1] == "supports" {
		// Only the HTML renderer shows SVGs.
		if len(os.Args) > 2 && os.Args[2] != "html" {
			os.Exit(1)
		}
		return
	}
	if err := run(os.Stdin, os.Stdout); err != nil {
		fmt.Fprintln(os.Stderr, "mdbook-pikchr:", err)
		os.Exit(1)
	}
}

func run(r io.Reader, w io.Writer) error {
	var input [2]json.RawMessage
	if err := json.NewDecoder(r).Decode(&input); err != nil {
		return err
	}
	var book any
	if err := json.Unmarshal(input[1], &book); err != nil {
		return err
	}
	if err := walk(book); err != nil {
		return err
	}
	return json.NewEncoder(w).Encode(book)
}

// walk draws the diagrams in every chapter, which are objects with a name and content
// wherever they are in the book, so it doesn't depend on how a version of mdBook nests them.
func walk(v any) error {
	switch v := v.(type) {
	case map[string]any:
		if content, ok := v["content"].(string); ok {
			if _, ok := v["name"]; ok {
				drawn, err := draw(content)
				if err != nil {
					return fmt.Errorf("%v: %w", v["name"], err)
				}
				v["content"] = drawn
			}
		}
		for _, child := range v {
			if err := walk(child); err != nil {
				return err
			}
		}
	case []any:
		for _, child := range v {
			if err := walk(child); err != nil {
				return err
			}
		}
	}
	return nil
}

// draw replaces each pikchr block in a chapter with its diagram.
func draw(content string) (string, error) {
	var out, src strings.Builder
	in := false
	for _, line := range strings.SplitAfter(content, "\n") {
		trimmed := strings.TrimSpace(line)
		switch {
		case !in && trimmed == "```pikchr":
			in = true
			src.Reset()
		case in && trimmed == "```":
			in = false
			svg, width, _, err := gopikchr.Convert(src.String(), gopikchr.WithPlaintextErrors())
			if err != nil {
				return "", fmt.Errorf("pikchr: %s", svg)
			}
			svg = strings.ReplaceAll(svg, "rgb(0,0,0)", "currentColor")
			// One line, so Markdown keeps it a single block of HTML.
			svg = strings.Join(strings.Fields(svg), " ")
			fmt.Fprintf(&out, "\n<figure class=\"diagram\" style=\"--diagram-width: %dpx\">%s</figure>\n\n", width, svg)
		case in:
			src.WriteString(line)
		default:
			out.WriteString(line)
		}
	}
	if in {
		return "", fmt.Errorf("pikchr: a block isn't closed")
	}
	return out.String(), nil
}
