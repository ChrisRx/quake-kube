package content

import (
	"archive/zip"
	"bytes"
	"image/jpeg"
	"os"
	"path/filepath"
	"testing"
)

// tga returns a 2x2 TGA image, run-length encoded when rle is set: red on the top row and
// blue on the bottom, stored from the bottom.
func tga(rle bool) []byte {
	header := make([]byte, 18)
	header[2] = 2
	header[12], header[14], header[16] = 2, 2, 24
	blue, red := []byte{255, 0, 0}, []byte{0, 0, 255}
	if !rle {
		return bytes.Join([][]byte{header, blue, blue, red, red}, nil)
	}
	header[2] = 10
	// A run of 2 blue, then 2 raw red.
	return bytes.Join([][]byte{header, {0x81}, blue, {0x01}, red, red}, nil)
}

func writePakFiles(t *testing.T, path string, files map[string][]byte) {
	t.Helper()
	if err := os.MkdirAll(filepath.Dir(path), 0755); err != nil {
		t.Fatal(err)
	}
	f, err := os.Create(path)
	if err != nil {
		t.Fatal(err)
	}
	defer f.Close()
	w := zip.NewWriter(f)
	for name, data := range files {
		fw, err := w.Create(name)
		if err != nil {
			t.Fatal(err)
		}
		if _, err := fw.Write(data); err != nil {
			t.Fatal(err)
		}
	}
	if err := w.Close(); err != nil {
		t.Fatal(err)
	}
}

func TestDecodeTGA(t *testing.T) {
	for _, rle := range []bool{false, true} {
		img, err := decodeTGA(tga(rle))
		if err != nil {
			t.Fatalf("rle %v: %v", rle, err)
		}
		r, _, b, _ := img.At(0, 0).RGBA()
		if r != 0xffff || b != 0 {
			t.Errorf("rle %v: top left is %v, want red", rle, img.At(0, 0))
		}
		r, _, b, _ = img.At(1, 1).RGBA()
		if r != 0 || b != 0xffff {
			t.Errorf("rle %v: bottom right is %v, want blue", rle, img.At(1, 1))
		}
	}
	if _, err := decodeTGA(tga(false)[:20]); err == nil {
		t.Error("decoding a truncated image succeeded")
	}
}

func TestReadLevelshot(t *testing.T) {
	dir := t.TempDir()
	writePakFiles(t, filepath.Join(dir, "baseq3", "pak0.pk3"), map[string][]byte{
		"levelshots/q3dm17.tga": tga(false),
		"levelshots/q3dm7.jpg":  []byte("base"),
	})
	// A mod's paks come after baseq3's.
	writePakFiles(t, filepath.Join(dir, "cpma", "z-maps.pk3"), map[string][]byte{
		"LevelShots/Q3DM7.JPG": []byte("mod"),
	})

	data, err := ReadLevelshot(dir, "q3dm7")
	if err != nil || string(data) != "mod" {
		t.Errorf("q3dm7: got %q, %v, want the mod's", data, err)
	}
	data, err = ReadLevelshot(dir, "q3dm17")
	if err != nil {
		t.Fatalf("q3dm17: %v", err)
	}
	if _, err := jpeg.Decode(bytes.NewReader(data)); err != nil {
		t.Errorf("q3dm17: converted levelshot isn't a JPEG: %v", err)
	}
	if _, err := ReadLevelshot(dir, "q3dm1"); err != ErrNoLevelshot {
		t.Errorf("q3dm1: got %v, want ErrNoLevelshot", err)
	}
}

func TestThumbnail(t *testing.T) {
	big, err := tgaToJPEG(tga(false))
	if err != nil {
		t.Fatal(err)
	}
	data, err := Thumbnail(big, 4, 3)
	if err != nil {
		t.Fatal(err)
	}
	img, err := jpeg.Decode(bytes.NewReader(data))
	if err != nil {
		t.Fatal(err)
	}
	if b := img.Bounds(); b.Dx() != 4 || b.Dy() != 3 {
		t.Errorf("thumbnail is %dx%d, want 4x3", b.Dx(), b.Dy())
	}
}
