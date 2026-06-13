package cleaner

import (
	"errors"
	"fmt"
	"io/fs"
	"os"
	"path/filepath"
	"runtime"
	"sort"
	"strings"
	"sync"
	"time"
)

type Options struct {
	DryRun bool
}

type Plan struct {
	Groups     []Group
	TotalBytes uint64
	Selected   int
}

type Group struct {
	App   string
	Label string
	Paths []string // resolved paths (globs expanded)
	Errs  []error
	Bytes uint64 // estimated reclaimable bytes
	On    bool   // selected for deletion
}

// AppGroup is used by the GUI to render the 2-level grouped list.
// Items are pointers into the Plan so toggling On is reflected immediately.
type AppGroup struct {
	App   string
	Items []*Group
	Bytes uint64 // sum of all items
}

// ByApp groups Plan.Groups by App name, sorted alphabetically.
func (p *Plan) ByApp() []AppGroup {
	order := make([]string, 0)
	byName := make(map[string][]*Group)
	for i := range p.Groups {
		g := &p.Groups[i]
		if _, seen := byName[g.App]; !seen {
			order = append(order, g.App)
		}
		byName[g.App] = append(byName[g.App], g)
	}
	sort.Strings(order)
	out := make([]AppGroup, 0, len(order))
	for _, app := range order {
		grps := byName[app]
		var total uint64
		for _, g := range grps {
			total += g.Bytes
		}
		out = append(out, AppGroup{App: app, Items: grps, Bytes: total})
	}
	return out
}

type ProgressUpdate struct {
	Phase   string
	Message string
	Current int
	Total   int
	Visited int
}

type PathError struct {
	Path  string `json:"path"`
	Error string `json:"error"`
}

type GroupResult struct {
	App            string      `json:"app"`
	Label          string      `json:"label"`
	Errors         []PathError `json:"errors,omitempty"`
	Bytes          uint64      `json:"bytes"`
	PathsAttempted int         `json:"paths_attempted"`
	PathsFailed    int         `json:"paths_failed"`
}

type ExecResult struct {
	StartedAt     time.Time     `json:"started_at"`
	FinishedAt    time.Time     `json:"finished_at"`
	Groups        []GroupResult `json:"groups"`
	SchemaVersion int           `json:"schema_version"`
	DurationMs    int64         `json:"duration_ms"`
	TotalSelected int           `json:"total_selected"`
	TotalBytes    uint64        `json:"total_bytes"`
	ErrorCount    int           `json:"error_count"`
	DryRun        bool          `json:"dry_run"`
}

func BuildPlanWithProgress(reg Registry, cb func(ProgressUpdate)) (Plan, error) {
	return buildPlan(reg, cb)
}

func buildPlan(reg Registry, cb func(ProgressUpdate)) (Plan, error) {
	if runtime.GOOS != "windows" {
		return Plan{}, errors.New("this tool only supports Windows")
	}

	groups := make([]Group, 0, len(reg.Items))
	for i, it := range reg.Items {
		resolved := make([]string, 0, len(it.Paths)+len(it.Globs))
		for _, p := range it.Paths {
			if p != "" {
				resolved = append(resolved, p)
			}
		}
		for _, g := range it.Globs {
			matches, err := filepath.Glob(g)
			if err != nil {
				return Plan{}, fmt.Errorf("failed to glob: %w", err)
			}
			resolved = append(resolved, matches...)
		}
		resolved = uniqueStrings(resolved)

		var total uint64
		var errs []error
		for _, p := range resolved {
			if !isSafePath(p) {
				errs = append(errs, fmt.Errorf("skipping unsafe path: %s", p))
				continue
			}
			b, err := dirSize(p)
			if err != nil {
				if !os.IsNotExist(err) {
					errs = append(errs, fmt.Errorf("%s: %w", p, err))
				}
				continue
			}
			total += b
		}

		groups = append(groups, Group{
			App:   it.App,
			Label: it.Label,
			Paths: resolved,
			On:    it.DefaultOn && total > 0, // never pre-select items with nothing to clean
			Bytes: total,
			Errs:  errs,
		})

		if cb != nil {
			cb(ProgressUpdate{
				Phase:   "scan",
				Current: i + 1,
				Total:   len(reg.Items),
				Message: fmt.Sprintf("%s - %s", it.App, it.Label),
			})
		}
	}

	// Append opt-in empty-folder removal groups (one per scanned root).
	groups = append(groups, buildEmptyFolderGroups()...)

	sort.SliceStable(groups, func(i, j int) bool {
		if groups[i].App == groups[j].App {
			return groups[i].Label < groups[j].Label
		}
		return groups[i].App < groups[j].App
	})

	var total uint64
	var selected int
	for _, g := range groups {
		if g.On {
			selected++
			total += g.Bytes
		}
	}

	return Plan{Groups: groups, TotalBytes: total, Selected: selected}, nil
}

func ExecuteWithResult(plan Plan, opts Options, cb func(ProgressUpdate)) (ExecResult, error) {
	return executeWithResult(plan, opts, cb)
}

func executeWithResult(plan Plan, opts Options, cb func(ProgressUpdate)) (ExecResult, error) {
	result := NewExecResult(plan, opts)
	if plan.Selected == 0 {
		finishExecResult(&result)
		return result, nil
	}

	var anyErr error
	total := 0
	for _, g := range plan.Groups {
		if g.On {
			total++
		}
	}
	current := 0
	for _, g := range plan.Groups {
		if !g.On {
			continue
		}
		current++
		if cb != nil {
			cb(ProgressUpdate{
				Phase:   "delete",
				Current: current,
				Total:   total,
				Message: fmt.Sprintf("%s - %s", g.App, g.Label),
			})
		}
		groupResult := GroupResult{
			App:   g.App,
			Label: g.Label,
			Bytes: g.Bytes,
		}
		for _, p := range g.Paths {
			if !isSafePath(p) {
				groupResult.Errors = append(groupResult.Errors, PathError{
					Path:  p,
					Error: "unsafe path (guard)",
				})
				continue
			}
			if _, err := os.Lstat(p); err != nil {
				continue
			}
			groupResult.PathsAttempted++
			if err := DeletePathSmart(p); err != nil {
				groupResult.PathsFailed++
				groupResult.Errors = append(groupResult.Errors, PathError{
					Path:  p,
					Error: err.Error(),
				})
				if anyErr == nil {
					anyErr = err
				}
			}
		}
		result.Groups = append(result.Groups, groupResult)
		result.ErrorCount += len(groupResult.Errors)
	}
	finishExecResult(&result)
	return result, anyErr
}

func uniqueStrings(in []string) []string {
	seen := map[string]struct{}{}
	out := make([]string, 0, len(in))
	for _, s := range in {
		s = filepath.Clean(s)
		low := strings.ToLower(s)
		if _, ok := seen[low]; ok {
			continue
		}
		seen[low] = struct{}{}
		out = append(out, s)
	}
	return out
}

// dirSize returns the total byte size of a path. Returns (0, nil) for
// non-existent paths to avoid noisy errors. Skips symlinks/reparse points.
func dirSize(p string) (uint64, error) {
	info, err := os.Lstat(p)
	if err != nil {
		if os.IsNotExist(err) {
			return 0, nil
		}
		return 0, err
	}
	if !info.IsDir() {
		return uint64(max(0, info.Size())), nil
	}

	var total uint64
	var mu sync.Mutex
	err = filepath.WalkDir(p, func(_ string, d fs.DirEntry, _ error) error {
		if d.Type()&os.ModeSymlink != 0 {
			if d.IsDir() {
				return filepath.SkipDir
			}
			return nil
		}
		if !d.IsDir() {
			fi, e := d.Info()
			if e == nil {
				mu.Lock()
				total += uint64(max(0, fi.Size()))
				mu.Unlock()
			}
		}
		return nil
	})
	return total, err
}

// isSafePath guards against deleting anything outside the four known safe roots,
// or the roots themselves.
func isSafePath(p string) bool {
	roots := []string{
		os.Getenv("LOCALAPPDATA"),
		os.Getenv("APPDATA"),
		os.Getenv("PROGRAMDATA"),
		os.Getenv("USERPROFILE"),
		// The registry also curates targets under the Windows and Program Files
		// trees (e.g. Prefetch, SoftwareDistribution\Download, Ubisoft launcher).
		os.Getenv("SystemRoot"),
		os.Getenv("windir"),
		os.Getenv("ProgramFiles"),
		os.Getenv("ProgramFiles(x86)"),
		os.Getenv("ProgramW6432"),
	}
	return isPathUnderAnyRoot(p, roots)
}

func isPathUnderAnyRoot(path string, roots []string) bool {
	for _, root := range roots {
		if isPathUnderRoot(path, root) {
			return true
		}
	}
	return false
}

func isPathUnderRoot(path, root string) bool {
	if path == "" || root == "" {
		return false
	}
	pathKey := normalizedPathKey(path)
	rootKey := normalizedPathKey(root)
	if pathKey == rootKey {
		return false
	}
	return strings.HasPrefix(pathKey, rootKey+string(filepath.Separator))
}

func normalizedPathKey(path string) string {
	return strings.ToLower(filepath.Clean(path))
}
