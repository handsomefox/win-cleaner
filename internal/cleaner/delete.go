package cleaner

import (
	"fmt"
	"os"
	"path/filepath"

	"win-clear/internal/trash"
)

// largeChildThreshold is the number of immediate children above which we
// attempt to move the whole directory in one shot rather than batching children.
// Shader caches and Battle.net folders can have tens of thousands of files;
// individually recycling hundreds of chunks of 64 is extremely slow on Windows.
const largeChildThreshold = 500

// chunkSize is the number of paths sent per SHFileOperationW call when
// processing directories below the large-child threshold.
const chunkSize = 64

// DeletePathSmart deletes path using only the Recycle Bin.
//   - File/link: attempt to move to Recycle Bin directly.
//   - Small directory (≤ largeChildThreshold children): batch children in
//     chunks then recycle the now-empty parent.
//   - Large directory (> largeChildThreshold children): try recycling the
//     whole folder in one call first (much faster); falls back to the chunk
//     approach only if that fails.
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

	// Directory: enumerate immediate children
	children, err := readDirNames(clean)
	if err != nil {
		if os.IsNotExist(err) {
			return nil
		}
		// Enumeration failed (permission issues); try the folder itself anyway.
		if e := recycleOne(clean); e != nil {
			fmt.Printf("  Could not recycle folder %s (%v).\n", clean, e)
			return e
		}
		return nil
	}

	// Large directory: try recycling the whole folder in one API call.
	// This avoids hundreds of SHFileOperationW calls for shader-cache and
	// similar directories with thousands of entries.
	if len(children) > largeChildThreshold {
		if err := recycleOne(clean); err == nil {
			return nil
		}
		// Whole-folder recycle failed (e.g. a child is locked); fall through
		// to the chunk approach so we still clean as much as possible.
	}

	// Batch children to Recycle Bin in chunks
	for i := 0; i < len(children); i += chunkSize {
		j := min(i+chunkSize, len(children))
		if err := trash.MoveToRecycleBin(children[i:j]); err != nil {
			// Bulk move failed; attempt per-item to salvage progress
			for _, c := range children[i:j] {
				if e := recycleOne(c); e != nil {
					fmt.Printf("  Recycle Bin move failed for %s (%v). Skipping.\n", c, e)
				}
			}
		}
	}

	// Recycle the (now hopefully empty) directory itself
	if err := recycleOne(clean); err != nil {
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
