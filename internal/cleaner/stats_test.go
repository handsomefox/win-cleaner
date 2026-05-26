package cleaner

import (
	"os"
	"path/filepath"
	"testing"
	"time"

	"github.com/stretchr/testify/require"
)

func TestLoadStatsEmpty(t *testing.T) {
	home := t.TempDir()
	t.Setenv("HOME", home)

	results, skipped, err := LoadStats()
	require.NoError(t, err)
	require.Empty(t, results)
	require.Zero(t, skipped)
}

func TestWriteAndLoadStats(t *testing.T) {
	home := t.TempDir()
	t.Setenv("HOME", home)

	finish := time.Now().UTC().Truncate(time.Millisecond)
	res := ExecResult{
		StartedAt:     finish.Add(-2 * time.Second),
		FinishedAt:    finish,
		DurationMs:    2000,
		DryRun:        false,
		TotalSelected: 2,
		TotalBytes:    1234,
	}

	path, err := WriteStats(&res)
	require.NoError(t, err)
	_, err = os.Stat(path)
	require.NoError(t, err)

	results, skipped, err := LoadStats()
	require.NoError(t, err)
	require.Zero(t, skipped)
	require.Len(t, results, 1)
	require.Equal(t, statsSchemaVersion, results[0].SchemaVersion)
	require.Equal(t, res.TotalSelected, results[0].TotalSelected)
	require.Equal(t, res.TotalBytes, results[0].TotalBytes)
}

func TestLoadStatsSkipsInvalidFiles(t *testing.T) {
	home := t.TempDir()
	t.Setenv("HOME", home)

	statsDir, err := statsDirectory()
	require.NoError(t, err)
	require.NoError(t, os.MkdirAll(statsDir, 0o700))
	badPath := filepath.Join(statsDir, "bad.json")
	require.NoError(t, os.WriteFile(badPath, []byte("{"), 0o600))

	results, skipped, err := LoadStats()
	require.NoError(t, err)
	require.Empty(t, results)
	require.Equal(t, 1, skipped)
}
