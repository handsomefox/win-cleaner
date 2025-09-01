package cleaner

import (
	"fmt"
	"os"
	"path/filepath"

	"win-clear/internal/trash"
)

// DeletePathSmart deletes path using only the Recycle Bin.
//   - File/link: attempt to move to Recycle Bin.
//   - Directory: first attempt to move all immediate children to Recycle Bin
//     then attempt to move the now-empty directory.
//   - If any move fails, we print a message and skip (no permanent deletion).
func DeletePathSmart(path string) error {
	clean := filepath.Clean(path)

	info, err := os.Lstat(clean)
	if err != nil {
		if os.IsNotExist(err) {
			return nil
		}
		// If we can't stat it, still try sending to Recycle Bin (may succeed).
		return recycleOne(clean)
	}

	// Non-directory (file or link)
	if !info.IsDir() {
		return recycleOne(clean)
	}

	// Directory: delete children first
	children, err := readDirNames(clean)
	if err != nil {
		if os.IsNotExist(err) {
			return nil
		}
		// If enumerating fails (permission issues), try the folder itself anyway.
		if e := recycleOne(clean); e != nil {
			fmt.Printf("  Could not recycle folder %s (%v).\n", clean, e)
			return e
		}
		return nil
	}

	// Batch children to Recycle Bin
	const chunk = 64
	for i := 0; i < len(children); i += chunk {
		j := i + chunk
		if j > len(children) {
			j = len(children)
		}
		if err := trash.MoveToRecycleBin(children[i:j]); err != nil {
			// If bulk move fails, attempt per-item to salvage progress
			for _, c := range children[i:j] {
				if e := recycleOne(c); e != nil {
					fmt.Printf("  Recycle Bin move failed for %s (%v). Skipping.\n", c, e)
				}
			}
		}
	}

	// Try to recycle the (now hopefully empty) directory itself
	if err := recycleOne(clean); err != nil {
		// Leave it if protected/in use
		fmt.Printf("  Could not recycle folder %s (%v). Leaving folder.\n", clean, err)
		return err
	}

	return nil
}

func recycleOne(p string) error {
	p = filepath.FromSlash(p)
	return trash.MoveToRecycleBin([]string{p})
}

// readDirNames returns absolute paths of immediate children of dir.
func readDirNames(dir string) ([]string, error) {
	entries, err := os.ReadDir(dir)
	if err != nil {
		return nil, err
	}
	out := make([]string, 0, len(entries))
	for _, e := range entries {
		out = append(out, filepath.Join(dir, e.Name()))
	}
	return out, nil
}
