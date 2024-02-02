package fs

import (
	"os"
	"path/filepath"
)

func WalkFiles(root string, walkFn filepath.WalkFunc, exts ...string) error {
	return filepath.Walk(root, func(path string, info os.FileInfo, err error) error {
		if err != nil {
			if os.IsPermission(err) {
				return nil
			}
			return err
		}
		if !HasExts(path, exts...) {
			return nil
		}
		if info.IsDir() {
			return nil
		}
		return walkFn(path, info, err)
	})
}
