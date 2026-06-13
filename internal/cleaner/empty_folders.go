package cleaner

import (
	"io"
	"io/fs"
	"os"
	"path/filepath"
	"sort"
)

// emptyScanBatch is the number of directory entries read per syscall. Reading in
// batches (rather than os.ReadDir, which slurps and sorts the whole directory)
// lets us bail on the first file without enumerating a huge folder.
const emptyScanBatch = 256

// EmptyFoldersApp is the App name used for the empty-folder removal groups.
// It is also referenced by the GUI to assign these groups to their category.
const EmptyFoldersApp = "Empty folders"

// emptyFolderRoot pairs a user-facing label with a resolved root path whose
// direct child folders are candidates for empty-folder removal.
type emptyFolderRoot struct {
	label string
	path  string
}

// emptyFolderRoots returns the four roots swept for empty top-level folders.
// Only the immediate children of these roots are ever considered for removal.
func emptyFolderRoots() []emptyFolderRoot {
	localAppData := os.Getenv("LOCALAPPDATA")
	appData := os.Getenv("APPDATA")
	programData := os.Getenv("PROGRAMDATA")
	userProfile := os.Getenv("USERPROFILE")

	roots := make([]emptyFolderRoot, 0, 4)
	add := func(label, path string) {
		if path != "" {
			roots = append(roots, emptyFolderRoot{label: label, path: path})
		}
	}
	add(`AppData\Local`, localAppData)
	if userProfile != "" {
		add(`AppData\LocalLow`, filepath.Join(userProfile, "AppData", "LocalLow"))
	}
	add(`AppData\Roaming`, appData)
	add("ProgramData", programData)
	return roots
}

// buildEmptyFolderGroups scans the direct children of each empty-folder root and
// returns one Group per root that has at least one qualifying folder. A folder
// qualifies when its entire subtree contains zero files (it is empty, or
// contains only nested empty folders); the whole top-level folder is then a
// candidate to be recycled. Reparse points (junctions/symlinks/cloud
// placeholders) are treated as content and are never followed.
//
// The resulting groups are pre-built and appended to the Plan, so they flow
// through the same selection/preview/recycle pipeline as registry groups. They
// carry Bytes: 0 and are never pre-selected (opt-in only).
func buildEmptyFolderGroups() []Group {
	groups := make([]Group, 0, len(emptyFolderRoots()))
	for _, root := range emptyFolderRoots() {
		entries, err := os.ReadDir(root.path)
		if err != nil {
			continue
		}
		var candidates []string
		for _, e := range entries {
			if !isPlainDir(e.Type()) {
				continue
			}
			child := filepath.Join(root.path, e.Name())
			if subtreeHasNoFiles(child) {
				candidates = append(candidates, child)
			}
		}
		if len(candidates) == 0 {
			continue
		}
		sort.Strings(candidates)
		groups = append(groups, Group{
			App:   EmptyFoldersApp,
			Label: root.label,
			Paths: candidates,
			Bytes: 0,
			On:    false,
		})
	}
	return groups
}

// isPlainDir reports whether mode describes a real directory that is not a
// reparse point (symlink, junction, mount point, or cloud placeholder).
func isPlainDir(mode fs.FileMode) bool {
	if mode&(fs.ModeSymlink|fs.ModeIrregular) != 0 {
		return false
	}
	return mode.IsDir()
}

// subtreeHasNoFiles reports whether dir's entire subtree contains zero files.
//
// It is written to bail as early as possible: entries are read in batches
// (never the whole directory at once), every batch is scanned for a
// non-directory entry before any subdirectory is descended into, and the first
// file or reparse point found anywhere returns false immediately. Deep recursion
// therefore only happens over genuinely empty directory trees. A directory it
// cannot read is treated as non-empty (returns false) so we never recycle on a
// read error.
func subtreeHasNoFiles(dir string) bool {
	f, err := os.Open(dir) //nolint:gosec // dir is confined to the known cache roots; deletion is separately guarded by isSafePath.
	if err != nil {
		return false
	}
	defer f.Close() //nolint:errcheck // read-only directory handle; a close error is not actionable.

	for {
		entries, readErr := f.ReadDir(emptyScanBatch)

		// Fail fast: any non-directory in this batch disqualifies dir before we
		// pay the cost of descending into the batch's subdirectories.
		subdirs := make([]string, 0, len(entries))
		for _, e := range entries {
			if !isPlainDir(e.Type()) {
				return false
			}
			subdirs = append(subdirs, filepath.Join(dir, e.Name()))
		}
		for _, sub := range subdirs {
			if !subtreeHasNoFiles(sub) {
				return false
			}
		}

		if readErr == io.EOF {
			return true
		}
		if readErr != nil {
			return false
		}
	}
}
