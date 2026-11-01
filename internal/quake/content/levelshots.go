package content

import (
	"archive/zip"
	"bytes"
	"encoding/binary"
	"errors"
	"fmt"
	"image"
	"image/color"
	"image/jpeg"
	"io"
	"os"
	"path"
	"path/filepath"
	"sort"
	"strings"
	"sync"
)

// ErrNoLevelshot is returned when none of the content's paks has a map's levelshot.
var ErrNoLevelshot = errors.New("no levelshot")

// levelshots caches each map's levelshot, or the lack of one. Content doesn't change while
// the server runs, since it's downloaded before the server starts.
var levelshots sync.Map

// ReadLevelshot returns a map's levelshot, the picture the game shows while loading it, as a
// JPEG. Many maps have a TGA one, which browsers can't show, so it's converted.
func ReadLevelshot(dir, mapName string) ([]byte, error) {
	key := strings.ToLower(mapName)
	if v, ok := levelshots.Load(key); ok {
		if v == nil {
			return nil, ErrNoLevelshot
		}
		return v.([]byte), nil
	}
	data, err := readLevelshot(dir, key)
	if errors.Is(err, ErrNoLevelshot) {
		levelshots.Store(key, nil)
	} else if err == nil {
		levelshots.Store(key, data)
	}
	return data, err
}

func readLevelshot(dir, mapName string) ([]byte, error) {
	var paks []string
	if err := walkFiles(dir, func(path string, _ os.FileInfo, _ error) error {
		paks = append(paks, path)
		return nil
	}, ".pk3"); err != nil {
		return nil, err
	}
	// The game reads files from the last pak that has them, with a mod's paks after
	// baseq3's.
	base := filepath.Join(dir, "baseq3") + string(filepath.Separator)
	sort.Slice(paks, func(i, j int) bool {
		bi, bj := strings.HasPrefix(paks[i], base), strings.HasPrefix(paks[j], base)
		if bi != bj {
			return bi
		}
		return paks[i] < paks[j]
	})
	for i := len(paks) - 1; i >= 0; i-- {
		data, err := readPakLevelshot(paks[i], mapName)
		if errors.Is(err, ErrNoLevelshot) {
			continue
		}
		if err != nil {
			return nil, fmt.Errorf("reading %s: %w", paks[i], err)
		}
		return data, nil
	}
	return nil, ErrNoLevelshot
}

func readPakLevelshot(pak, mapName string) ([]byte, error) {
	r, err := zip.OpenReader(pak)
	if err != nil {
		return nil, err
	}
	defer r.Close()
	for _, f := range r.File {
		name := strings.ToLower(f.Name)
		if path.Dir(name) != "levelshots" {
			continue
		}
		ext := path.Ext(name)
		if strings.TrimSuffix(path.Base(name), ext) != mapName {
			continue
		}
		var decode func([]byte) ([]byte, error)
		switch ext {
		case ".jpg", ".jpeg":
			decode = func(data []byte) ([]byte, error) { return data, nil }
		case ".tga":
			decode = tgaToJPEG
		default:
			continue
		}
		rc, err := f.Open()
		if err != nil {
			return nil, err
		}
		data, err := io.ReadAll(io.LimitReader(rc, 16<<20))
		rc.Close()
		if err != nil {
			return nil, err
		}
		return decode(data)
	}
	return nil, ErrNoLevelshot
}

// Thumbnail scales a JPEG to width×height, averaging the pixels each one covers.
func Thumbnail(data []byte, width, height int) ([]byte, error) {
	src, err := jpeg.Decode(bytes.NewReader(data))
	if err != nil {
		return nil, err
	}
	b := src.Bounds()
	dst := image.NewRGBA(image.Rect(0, 0, width, height))
	for y := range height {
		y0 := b.Min.Y + y*b.Dy()/height
		y1 := max(b.Min.Y+(y+1)*b.Dy()/height, y0+1)
		for x := range width {
			x0 := b.Min.X + x*b.Dx()/width
			x1 := max(b.Min.X+(x+1)*b.Dx()/width, x0+1)
			var r, g, bl, n uint32
			for sy := y0; sy < y1; sy++ {
				for sx := x0; sx < x1; sx++ {
					cr, cg, cb, _ := src.At(sx, sy).RGBA()
					r, g, bl, n = r+cr, g+cg, bl+cb, n+1
				}
			}
			dst.SetRGBA(x, y, color.RGBA{uint8(r / n >> 8), uint8(g / n >> 8), uint8(bl / n >> 8), 255})
		}
	}
	var out bytes.Buffer
	if err := jpeg.Encode(&out, dst, &jpeg.Options{Quality: 80}); err != nil {
		return nil, err
	}
	return out.Bytes(), nil
}

func tgaToJPEG(data []byte) ([]byte, error) {
	img, err := decodeTGA(data)
	if err != nil {
		return nil, err
	}
	var b bytes.Buffer
	if err := jpeg.Encode(&b, img, &jpeg.Options{Quality: 85}); err != nil {
		return nil, err
	}
	return b.Bytes(), nil
}

// decodeTGA decodes the kinds of TGA images the game reads: true color or grayscale,
// uncompressed or run-length encoded.
func decodeTGA(data []byte) (image.Image, error) {
	if len(data) < 18 {
		return nil, errors.New("tga: short header")
	}
	idLength := int(data[0])
	colorMapType := data[1]
	imageType := data[2]
	width := int(binary.LittleEndian.Uint16(data[12:]))
	height := int(binary.LittleEndian.Uint16(data[14:]))
	bpp := int(data[16])
	topDown := data[17]&0x20 != 0
	if colorMapType != 0 {
		return nil, errors.New("tga: color-mapped images aren't supported")
	}
	rle := imageType == 10 || imageType == 11
	gray := imageType == 3 || imageType == 11
	switch {
	case gray && bpp == 8, !gray && (imageType == 2 || imageType == 10) && (bpp == 24 || bpp == 32):
	default:
		return nil, fmt.Errorf("tga: unsupported image type %d with %d bits per pixel", imageType, bpp)
	}
	if width == 0 || height == 0 || width > 4096 || height > 4096 {
		return nil, fmt.Errorf("tga: unsupported size %dx%d", width, height)
	}
	src := data[min(18+idLength, len(data)):]
	size := bpp / 8
	n := width * height * size
	pixels := make([]byte, 0, n)
	if rle {
		for len(pixels) < n {
			if len(src) < 1 {
				return nil, io.ErrUnexpectedEOF
			}
			header := src[0]
			src = src[1:]
			count := int(header&0x7f) + 1
			if header&0x80 != 0 {
				// A run of one pixel repeated.
				if len(src) < size {
					return nil, io.ErrUnexpectedEOF
				}
				for range count {
					pixels = append(pixels, src[:size]...)
				}
				src = src[size:]
			} else {
				if len(src) < count*size {
					return nil, io.ErrUnexpectedEOF
				}
				pixels = append(pixels, src[:count*size]...)
				src = src[count*size:]
			}
		}
		// The last run can go past the end.
		pixels = pixels[:n]
	} else {
		if len(src) < n {
			return nil, io.ErrUnexpectedEOF
		}
		pixels = src[:n]
	}

	img := image.NewNRGBA(image.Rect(0, 0, width, height))
	for y := range height {
		// Rows are stored from the bottom unless the image says otherwise.
		row := y
		if !topDown {
			row = height - 1 - y
		}
		for x := range width {
			p := pixels[(y*width+x)*size:]
			var c color.NRGBA
			switch size {
			case 1:
				c = color.NRGBA{p[0], p[0], p[0], 255}
			case 3:
				c = color.NRGBA{p[2], p[1], p[0], 255}
			case 4:
				c = color.NRGBA{p[2], p[1], p[0], p[3]}
			}
			img.SetNRGBA(x, row, c)
		}
	}
	return img, nil
}
