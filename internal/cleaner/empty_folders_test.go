package cleaner

import (
	"context"
	"os"
	"path/filepath"
	"runtime"
	"testing"

	"github.com/stretchr/testify/require"
)

func TestScanEmptyFoldersIncludesEmptyDirectory(t *testing.T) {
	root := t.TempDir()
	empty := filepath.Join(root, "empty")
	require.NoError(t, os.MkdirAll(empty, 0o750))

	result := scanEmptyFoldersWithOptions(context.Background(), root, EmptyFolderScanOptions{}, nil)
	require.Empty(t, result.Errs)
	require.Equal(t, []string{empty}, result.Candidates)
}

func TestScanEmptyFoldersTreatsEmptyNestedFoldersAsOneCandidate(t *testing.T) {
	root := t.TempDir()
	top := filepath.Join(root, "top")
	require.NoError(t, os.MkdirAll(filepath.Join(top, "child", "grandchild"), 0o750))

	result := scanEmptyFoldersWithOptions(context.Background(), root, EmptyFolderScanOptions{}, nil)
	require.Empty(t, result.Errs)
	require.Equal(t, []string{top}, result.Candidates)
}

func TestScanEmptyFoldersDoesNotIncludeDirectoryWithZeroByteFile(t *testing.T) {
	root := t.TempDir()
	dir := filepath.Join(root, "dir")
	require.NoError(t, os.MkdirAll(dir, 0o750))
	require.NoError(t, os.WriteFile(filepath.Join(dir, "empty.txt"), nil, 0o600))

	result := scanEmptyFoldersWithOptions(context.Background(), root, EmptyFolderScanOptions{}, nil)
	require.Empty(t, result.Errs)
	require.Empty(t, result.Candidates)
}

func TestScanEmptyFoldersDoesNotIncludeDirectoryWithDeepFile(t *testing.T) {
	root := t.TempDir()
	dir := filepath.Join(root, "dir")
	child := filepath.Join(dir, "child")
	require.NoError(t, os.MkdirAll(child, 0o750))
	require.NoError(t, os.WriteFile(filepath.Join(child, "file.txt"), []byte("x"), 0o600))

	result := scanEmptyFoldersWithOptions(context.Background(), root, EmptyFolderScanOptions{}, nil)
	require.Empty(t, result.Errs)
	require.Empty(t, result.Candidates)
}

func TestScanEmptyFoldersDoesNotIncludeRoot(t *testing.T) {
	root := t.TempDir()

	result := scanEmptyFoldersWithOptions(context.Background(), root, EmptyFolderScanOptions{}, nil)
	require.Empty(t, result.Errs)
	require.Empty(t, result.Candidates)
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

	result := scanEmptyFoldersWithOptions(context.Background(), root, EmptyFolderScanOptions{}, nil)
	require.Empty(t, result.Errs)
	require.Equal(t, []string{target}, result.Candidates)
}

func TestScanEmptyFoldersReportsUnreadableDirectory(t *testing.T) {
	if runtime.GOOS == "windows" {
		t.Skip("chmod-based unreadable directory setup is not reliable on Windows")
	}
	root := t.TempDir()
	blocked := filepath.Join(root, "blocked")
	require.NoError(t, os.MkdirAll(blocked, 0o750))
	require.NoError(t, os.Chmod(blocked, 0))
	t.Cleanup(func() {
		require.NoError(t, os.Chmod(blocked, 0o400))
	})

	result := scanEmptyFoldersWithOptions(context.Background(), root, EmptyFolderScanOptions{}, nil)
	require.Empty(t, result.Candidates)
	require.NotEmpty(t, result.Errs)
	require.Equal(t, blocked, result.Errs[0].Path)
}

func TestBuildEmptyFolderPlanDeduplicatesOverlappingRoots(t *testing.T) {
	root := t.TempDir()
	child := filepath.Join(root, "child")
	require.NoError(t, os.MkdirAll(child, 0o750))

	plan := BuildEmptyFolderPlanWithCancel([]EmptyFolderRoot{
		{Label: "parent", Path: root, On: true},
		{Label: "child", Path: child, On: true},
	}, nil, nil)

	require.False(t, plan.Cancelled)
	require.Equal(t, []EmptyFolderCandidate{{Path: child, On: true}}, plan.Folders)
}

func TestScanEmptyFoldersCancellationStopsScan(t *testing.T) {
	root := t.TempDir()
	require.NoError(t, os.MkdirAll(filepath.Join(root, "child"), 0o750))
	ctx, cancel := context.WithCancel(context.Background())
	cancel()

	result := scanEmptyFoldersWithOptions(ctx, root, EmptyFolderScanOptions{}, nil)
	require.True(t, result.Cancelled)
	require.Empty(t, result.Candidates)
}

func TestScanEmptyFoldersRetainsBoundedCandidates(t *testing.T) {
	root := t.TempDir()
	for _, name := range []string{"a", "b", "c"} {
		require.NoError(t, os.MkdirAll(filepath.Join(root, name), 0o750))
	}

	result := scanEmptyFoldersWithOptions(context.Background(), root, EmptyFolderScanOptions{MaxCandidates: 2}, nil)
	require.True(t, result.CandidateLimitHit)
	require.Len(t, result.Candidates, 2)
}

func TestScanEmptyFoldersRetainsBoundedErrors(t *testing.T) {
	if runtime.GOOS == "windows" {
		t.Skip("chmod-based unreadable directory setup is not reliable on Windows")
	}
	root := t.TempDir()
	for _, name := range []string{"a", "b"} {
		blocked := filepath.Join(root, name)
		require.NoError(t, os.MkdirAll(blocked, 0o750))
		require.NoError(t, os.Chmod(blocked, 0))
		t.Cleanup(func() {
			require.NoError(t, os.Chmod(blocked, 0o400))
		})
	}

	result := scanEmptyFoldersWithOptions(context.Background(), root, EmptyFolderScanOptions{MaxErrors: 1}, nil)
	require.True(t, result.ErrorLimitHit)
	require.Len(t, result.Errs, 1)
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
