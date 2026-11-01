package content

import (
	"encoding/binary"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"os"
	"path/filepath"
	"strings"
	"sync"
)

// PakIndex says where everything is in a pak, so browser clients can fetch only the parts the
// engine reads instead of the whole pak.
type PakIndex struct {
	Size int64 `json:"size"`
	// Directory is the range with the central directory and the end of the pak, which the
	// engine reads to list the pak's files.
	Directory [2]int64 `json:"directory"`
	// Files has each file's range, its local header followed by its data.
	Files map[string][2]int64 `json:"files"`
}

const (
	endOfCentralDirSig = 0x06054b50
	centralDirSig      = 0x02014b50
	localHeaderSig     = 0x04034b50
	endOfCentralDirLen = 22
	centralDirLen      = 46
	localHeaderLen     = 30

	// The engine looks for the end of the central directory in blocks of this size, starting
	// at the end of the pak.
	engineSearchBlock = 0x400 + 4
)

// pakIndexes caches indexes by the same key as scanned.
var pakIndexes sync.Map

// ReadPakIndex reads a pak's index from its zip structures.
func ReadPakIndex(path string) (*PakIndex, error) {
	info, err := os.Stat(path)
	if err != nil {
		return nil, err
	}
	key := checksumKey{path, info.Size(), info.ModTime()}
	if v, ok := pakIndexes.Load(key); ok {
		return v.(*PakIndex), nil
	}
	f, err := os.Open(path)
	if err != nil {
		return nil, err
	}
	defer f.Close()
	index, err := readPakIndex(f, info.Size())
	if err != nil {
		return nil, fmt.Errorf("reading %s: %w", path, err)
	}
	pakIndexes.Store(key, index)
	return index, nil
}

func readPakIndex(r io.ReaderAt, size int64) (*PakIndex, error) {
	// The end of the central directory is followed by a comment of up to 64 KB.
	tailLen := min(size, endOfCentralDirLen+0xffff)
	tail := make([]byte, tailLen)
	if _, err := r.ReadAt(tail, size-tailLen); err != nil {
		return nil, err
	}
	end := -1
	for i := len(tail) - endOfCentralDirLen; i >= 0; i-- {
		if binary.LittleEndian.Uint32(tail[i:]) == endOfCentralDirSig {
			end = i
			break
		}
	}
	if end < 0 {
		return nil, errors.New("no end of central directory")
	}
	eocd := tail[end:]
	count := int(binary.LittleEndian.Uint16(eocd[10:]))
	dirSize := int64(binary.LittleEndian.Uint32(eocd[12:]))
	dirOffset := int64(binary.LittleEndian.Uint32(eocd[16:]))
	if dirOffset+dirSize > size {
		return nil, errors.New("central directory out of range")
	}
	dir := make([]byte, dirSize)
	if _, err := r.ReadAt(dir, dirOffset); err != nil {
		return nil, err
	}

	index := &PakIndex{
		Size:      size,
		Directory: [2]int64{min(dirOffset, max(0, size-engineSearchBlock)), size},
		Files:     make(map[string][2]int64, count),
	}
	local := make([]byte, localHeaderLen)
	for range count {
		if len(dir) < centralDirLen || binary.LittleEndian.Uint32(dir) != centralDirSig {
			return nil, errors.New("invalid central directory")
		}
		compressed := int64(binary.LittleEndian.Uint32(dir[20:]))
		nameLen := int(binary.LittleEndian.Uint16(dir[28:]))
		extraLen := int(binary.LittleEndian.Uint16(dir[30:]))
		commentLen := int(binary.LittleEndian.Uint16(dir[32:]))
		offset := int64(binary.LittleEndian.Uint32(dir[42:]))
		recordLen := centralDirLen + nameLen + extraLen + commentLen
		if len(dir) < recordLen {
			return nil, errors.New("invalid central directory")
		}
		name := string(dir[centralDirLen : centralDirLen+nameLen])
		dir = dir[recordLen:]

		// The local header's name and extra field can differ in length from the central
		// directory's, so it's read for where the data starts.
		if _, err := r.ReadAt(local, offset); err != nil {
			return nil, err
		}
		if binary.LittleEndian.Uint32(local) != localHeaderSig {
			return nil, fmt.Errorf("invalid local header for %s", name)
		}
		dataStart := offset + localHeaderLen +
			int64(binary.LittleEndian.Uint16(local[26:])) +
			int64(binary.LittleEndian.Uint16(local[28:]))
		dataEnd := dataStart + compressed
		if dataEnd > size {
			return nil, fmt.Errorf("%s out of range", name)
		}
		index.Files[name] = [2]int64{offset, dataEnd}
	}
	return index, nil
}

// Limits on a ranges request, which a page makes for one batch of files.
const (
	maxRanges     = 16384
	maxRangeBytes = 512 << 20
)

// OpenRanges returns a reader of a file's ranges one after the other, and its length.
func OpenRanges(path string, ranges [][2]int64) (io.ReadCloser, int64, error) {
	if len(ranges) > maxRanges {
		return nil, 0, fmt.Errorf("at most %d ranges", maxRanges)
	}
	f, err := os.Open(path)
	if err != nil {
		return nil, 0, err
	}
	info, err := f.Stat()
	if err != nil {
		f.Close()
		return nil, 0, err
	}
	var total int64
	readers := make([]io.Reader, 0, len(ranges))
	for _, r := range ranges {
		if r[0] < 0 || r[0] >= r[1] || r[1] > info.Size() {
			f.Close()
			return nil, 0, fmt.Errorf("invalid range %d-%d", r[0], r[1])
		}
		total += r[1] - r[0]
		readers = append(readers, io.NewSectionReader(f, r[0], r[1]-r[0]))
	}
	if total > maxRangeBytes {
		f.Close()
		return nil, 0, fmt.Errorf("at most %d bytes", maxRangeBytes)
	}
	return readCloser{io.MultiReader(readers...), f}, total, nil
}

type readCloser struct {
	io.Reader
	io.Closer
}

// ProfileName is the name of the files in a game's directory saying which files the engine
// reads, so browser clients can fetch them before starting it. Content that adds to another's
// directory, like a map pack for a mod, names its own with a suffix, e.g.
// quake-kube-profile-maps.json.
const ProfileName = "quake-kube-profile.json"

// Profile lists the files the engine reads, by the pak they're in, e.g.
// "q3ut4/zUrT43_008.pk3": ["scripts/urban_terror.shader"].
type Profile struct {
	// Common has the files read on most maps.
	Common map[string][]string `json:"common"`
	// Maps has each map's other files.
	Maps map[string]map[string][]string `json:"maps"`
}

// ReadProfiles merges the profiles in a directory's games, e.g. a mod's and a map pack's.
func ReadProfiles(dir string) (*Profile, error) {
	merged := &Profile{Common: map[string][]string{}, Maps: map[string]map[string][]string{}}
	entries, err := os.ReadDir(dir)
	if err != nil {
		return nil, err
	}
	for _, entry := range entries {
		if !entry.IsDir() {
			continue
		}
		pattern := filepath.Join(dir, entry.Name(), strings.TrimSuffix(ProfileName, ".json")+"*.json")
		paths, err := filepath.Glob(pattern)
		if err != nil {
			return nil, err
		}
		for _, path := range paths {
			data, err := os.ReadFile(path)
			if err != nil {
				return nil, err
			}
			var p Profile
			if err := json.Unmarshal(data, &p); err != nil {
				return nil, fmt.Errorf("reading %s: %w", path, err)
			}
			mergeFiles(merged.Common, p.Common)
			for m, files := range p.Maps {
				if merged.Maps[m] == nil {
					merged.Maps[m] = map[string][]string{}
				}
				mergeFiles(merged.Maps[m], files)
			}
		}
	}
	return merged, nil
}

func mergeFiles(dst, src map[string][]string) {
	for pak, files := range src {
		dst[pak] = append(dst[pak], files...)
	}
}
