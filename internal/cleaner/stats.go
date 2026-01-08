package cleaner

import (
	"encoding/json"
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"time"
)

const statsSchemaVersion = 1

func WriteStats(result *ExecResult) (string, error) {
	if result == nil {
		return "", errors.New("stats result is nil")
	}
	statsDir, err := statsDirectory()
	if err != nil {
		return "", err
	}
	if err := os.MkdirAll(statsDir, 0o700); err != nil {
		return "", err
	}
	if result.FinishedAt.IsZero() {
		result.FinishedAt = time.Now()
	}
	result.SchemaVersion = statsSchemaVersion

	name := fmt.Sprintf("%s-%03d.json", result.FinishedAt.Format("20060102-150405"), result.FinishedAt.Nanosecond()/1e6)
	path := filepath.Join(statsDir, name)
	data, err := json.MarshalIndent(result, "", "  ")
	if err != nil {
		return "", err
	}
	if err := os.WriteFile(path, data, 0o600); err != nil {
		return "", err
	}
	return path, nil
}

func LoadStats() ([]ExecResult, int, error) {
	statsDir, err := statsDirectory()
	if err != nil {
		return nil, 0, err
	}
	entries, err := os.ReadDir(statsDir)
	if err != nil {
		if os.IsNotExist(err) {
			return nil, 0, nil
		}
		return nil, 0, err
	}
	results := make([]ExecResult, 0, len(entries))
	skipped := 0
	for _, entry := range entries {
		if entry.IsDir() {
			continue
		}
		name := entry.Name()
		if filepath.Ext(name) != ".json" {
			continue
		}
		path := filepath.Join(statsDir, name)
		// #nosec G304 -- reading known stats files from the stats directory.
		data, err := os.ReadFile(path)
		if err != nil {
			skipped++
			continue
		}
		var res ExecResult
		if err := json.Unmarshal(data, &res); err != nil {
			skipped++
			continue
		}
		results = append(results, res)
	}
	return results, skipped, nil
}

func statsDirectory() (string, error) {
	home, err := os.UserHomeDir()
	if err != nil {
		return "", err
	}
	return filepath.Join(home, ".win-cleaner", "stats"), nil
}

func NewExecResult(plan Plan, opts Options) ExecResult {
	return ExecResult{
		StartedAt:     time.Now(),
		DryRun:        opts.DryRun,
		TotalSelected: plan.Selected,
		TotalBytes:    plan.TotalBytes,
	}
}

func finishExecResult(result *ExecResult) {
	result.FinishedAt = time.Now()
	result.DurationMs = result.FinishedAt.Sub(result.StartedAt).Milliseconds()
}
