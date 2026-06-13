package cleaner

import (
	"os"
	"path/filepath"
	"testing"
)

func TestSubtreeHasNoFiles(t *testing.T) {
	root := t.TempDir()

	mustMkdirAll(t, filepath.Join(root, "empty"))
	mustMkdirAll(t, filepath.Join(root, "nestedEmpty", "a", "b"))
	mustMkdirAll(t, filepath.Join(root, "nestedEmpty", "c"))
	mustMkdirAll(t, filepath.Join(root, "withFile", "deep"))
	mustWriteFile(t, filepath.Join(root, "withFile", "deep", "f.txt"))
	mustMkdirAll(t, filepath.Join(root, "withLink"))
	if err := os.Symlink(filepath.Join(root, "empty"), filepath.Join(root, "withLink", "link")); err != nil {
		t.Skipf("symlinks unsupported on this platform: %v", err)
	}

	cases := map[string]bool{
		"empty":       true,
		"nestedEmpty": true,
		"withFile":    false, // contains a file somewhere in the subtree
		"withLink":    false, // reparse points count as content, never followed
	}
	for name, want := range cases {
		if got := subtreeHasNoFiles(filepath.Join(root, name)); got != want {
			t.Errorf("subtreeHasNoFiles(%q) = %v, want %v", name, got, want)
		}
	}

	// An unreadable / nonexistent directory must not be reported as empty.
	if subtreeHasNoFiles(filepath.Join(root, "does-not-exist")) {
		t.Errorf("subtreeHasNoFiles on missing dir = true, want false")
	}
}

func TestBuildEmptyFolderGroups(t *testing.T) {
	local := t.TempDir()
	// Isolate from the host environment: only LOCALAPPDATA is populated.
	t.Setenv("LOCALAPPDATA", local)
	t.Setenv("APPDATA", "")
	t.Setenv("PROGRAMDATA", "")
	t.Setenv("USERPROFILE", "")

	mustMkdirAll(t, filepath.Join(local, "EmptyApp"))
	mustMkdirAll(t, filepath.Join(local, "OnlyEmptyDirs", "sub"))
	mustMkdirAll(t, filepath.Join(local, "FullApp"))
	mustWriteFile(t, filepath.Join(local, "FullApp", "data.bin"))

	groups := buildEmptyFolderGroups()
	if len(groups) != 1 {
		t.Fatalf("len(groups) = %d, want 1 (only AppData\\Local populated)", len(groups))
	}
	g := groups[0]
	if g.App != EmptyFoldersApp || g.Label != `AppData\Local` {
		t.Fatalf("group identity = %q/%q, want %q/%q", g.App, g.Label, EmptyFoldersApp, `AppData\Local`)
	}
	if g.On || g.Bytes != 0 {
		t.Errorf("group On=%v Bytes=%d, want false/0 (opt-in, zero-size)", g.On, g.Bytes)
	}
	want := map[string]bool{
		filepath.Join(local, "EmptyApp"):      true,
		filepath.Join(local, "OnlyEmptyDirs"): true,
	}
	if len(g.Paths) != len(want) {
		t.Fatalf("paths = %v, want %v", g.Paths, want)
	}
	for _, p := range g.Paths {
		if !want[p] {
			t.Errorf("unexpected candidate %q (FullApp should be excluded)", p)
		}
	}
}

func mustMkdirAll(t *testing.T, path string) {
	t.Helper()
	if err := os.MkdirAll(path, 0o755); err != nil {
		t.Fatalf("mkdir %q: %v", path, err)
	}
}

func mustWriteFile(t *testing.T, path string) {
	t.Helper()
	if err := os.WriteFile(path, []byte("x"), 0o644); err != nil {
		t.Fatalf("write %q: %v", path, err)
	}
}
