package cleaner

import (
	"os"
	"path/filepath"
	"testing"

	"github.com/stretchr/testify/require"
)

func TestScanEmptyFoldersIncludesEmptyDirectory(t *testing.T) {
	root := t.TempDir()
	empty := filepath.Join(root, "empty")
	require.NoError(t, os.MkdirAll(empty, 0o750))

	got, errs := scanEmptyFolders(root)
	require.Empty(t, errs)
	require.Equal(t, []string{empty}, got)
}

func TestScanEmptyFoldersTreatsEmptyNestedFoldersAsOneCandidate(t *testing.T) {
	root := t.TempDir()
	top := filepath.Join(root, "top")
	require.NoError(t, os.MkdirAll(filepath.Join(top, "child", "grandchild"), 0o750))

	got, errs := scanEmptyFolders(root)
	require.Empty(t, errs)
	require.Equal(t, []string{top}, got)
}

func TestScanEmptyFoldersDoesNotIncludeDirectoryWithZeroByteFile(t *testing.T) {
	root := t.TempDir()
	dir := filepath.Join(root, "dir")
	require.NoError(t, os.MkdirAll(dir, 0o750))
	require.NoError(t, os.WriteFile(filepath.Join(dir, "empty.txt"), nil, 0o600))

	got, errs := scanEmptyFolders(root)
	require.Empty(t, errs)
	require.Empty(t, got)
}

func TestScanEmptyFoldersDoesNotIncludeDirectoryWithDeepFile(t *testing.T) {
	root := t.TempDir()
	dir := filepath.Join(root, "dir")
	child := filepath.Join(dir, "child")
	require.NoError(t, os.MkdirAll(child, 0o750))
	require.NoError(t, os.WriteFile(filepath.Join(child, "file.txt"), []byte("x"), 0o600))

	got, errs := scanEmptyFolders(root)
	require.Empty(t, errs)
	require.Empty(t, got)
}

func TestScanEmptyFoldersDoesNotIncludeRoot(t *testing.T) {
	root := t.TempDir()

	got, errs := scanEmptyFolders(root)
	require.Empty(t, errs)
	require.Empty(t, got)
}

func TestScanEmptyFoldersTreatsSymlinkAsBlocking(t *testing.T) {
	root := t.TempDir()
	dir := filepath.Join(root, "dir")
	target := filepath.Join(root, "target")
	link := filepath.Join(dir, "link")
	require.NoError(t, os.MkdirAll(dir, 0o750))
	require.NoError(t, os.MkdirAll(target, 0o750))
	if err := os.Symlink(target, link); err != nil {
		t.Skipf("symlinks unavailable: %v", err)
	}

	got, errs := scanEmptyFolders(root)
	require.Empty(t, errs)
	require.Equal(t, []string{target}, got)
}

func TestIsSafeEmptyCandidateRejectsRootItself(t *testing.T) {
	root := t.TempDir()
	roots := []EmptyFolderRoot{{Path: root, On: true}}

	require.False(t, isSafeEmptyCandidate(root, roots))
	require.True(t, isSafeEmptyCandidate(filepath.Join(root, "child"), roots))
}

func TestIsRecursivelyEmptyDirRechecksFilePresence(t *testing.T) {
	root := t.TempDir()
	dir := filepath.Join(root, "dir")
	require.NoError(t, os.MkdirAll(filepath.Join(dir, "child"), 0o750))

	empty, err := isRecursivelyEmptyDir(dir)
	require.NoError(t, err)
	require.True(t, empty)

	require.NoError(t, os.WriteFile(filepath.Join(dir, "child", "file.txt"), []byte("x"), 0o600))
	empty, err = isRecursivelyEmptyDir(dir)
	require.NoError(t, err)
	require.False(t, empty)
}
