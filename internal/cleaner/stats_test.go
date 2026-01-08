package cleaner

import (
	"os"
	"path/filepath"
	"testing"
	"time"
)

func TestLoadStatsEmpty(t *testing.T) {
	home := t.TempDir()
	t.Setenv("HOME", home)

	results, skipped, err := LoadStats()
	if err != nil {
		t.Fatalf("LoadStats returned error: %v", err)
	}
	if len(results) != 0 {
		t.Fatalf("expected no results, got %d", len(results))
	}
	if skipped != 0 {
		t.Fatalf("expected no skipped files, got %d", skipped)
	}
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
	if err != nil {
		t.Fatalf("WriteStats returned error: %v", err)
	}
	if _, err := os.Stat(path); err != nil {
		t.Fatalf("expected stats file to exist: %v", err)
	}

	results, skipped, err := LoadStats()
	if err != nil {
		t.Fatalf("LoadStats returned error: %v", err)
	}
	if skipped != 0 {
		t.Fatalf("expected no skipped files, got %d", skipped)
	}
	if len(results) != 1 {
		t.Fatalf("expected 1 result, got %d", len(results))
	}
	if results[0].SchemaVersion != statsSchemaVersion {
		t.Fatalf("expected schema version %d, got %d", statsSchemaVersion, results[0].SchemaVersion)
	}
	if results[0].TotalSelected != res.TotalSelected {
		t.Fatalf("expected total selected %d, got %d", res.TotalSelected, results[0].TotalSelected)
	}
	if results[0].TotalBytes != res.TotalBytes {
		t.Fatalf("expected total bytes %d, got %d", res.TotalBytes, results[0].TotalBytes)
	}
}

func TestLoadStatsSkipsInvalidFiles(t *testing.T) {
	home := t.TempDir()
	t.Setenv("HOME", home)

	statsDir, err := statsDirectory()
	if err != nil {
		t.Fatalf("statsDirectory returned error: %v", err)
	}
	if err := os.MkdirAll(statsDir, 0o700); err != nil {
		t.Fatalf("failed to create stats dir: %v", err)
	}
	badPath := filepath.Join(statsDir, "bad.json")
	if err := os.WriteFile(badPath, []byte("{"), 0o600); err != nil {
		t.Fatalf("failed to write bad stats file: %v", err)
	}

	results, skipped, err := LoadStats()
	if err != nil {
		t.Fatalf("LoadStats returned error: %v", err)
	}
	if len(results) != 0 {
		t.Fatalf("expected no results, got %d", len(results))
	}
	if skipped != 1 {
		t.Fatalf("expected 1 skipped file, got %d", skipped)
	}
}
