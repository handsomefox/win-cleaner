package cleaner

import (
	"context"
	"errors"
	"os"
	"path/filepath"
	"sort"
	"strings"
	"time"
)

const (
	defaultEmptyScanMaxErrors     = 100
	defaultEmptyScanMaxCandidates = 10000
)

type EmptyFolderRoot struct {
	Label string
	Path  string
	On    bool
}

type EmptyFolderPlan struct {
	Roots             []EmptyFolderRoot
	Folders           []EmptyFolderCandidate
	Errs              []PathError
	Selected          int
	VisitedDirs       int
	Cancelled         bool
	ErrorLimitHit     bool
	CandidateLimitHit bool
	StartedAt         time.Time
	FinishedAt        time.Time
}

type EmptyFolderCandidate struct {
	Path string
	On   bool
}

type EmptyFolderResult struct {
	StartedAt     time.Time
	FinishedAt    time.Time
	DurationMs    int64
	TotalSelected int
	Removed       int
	Failed        int
	Errors        []PathError
}

type EmptyFolderScanOptions struct {
	MaxErrors     int
	MaxCandidates int
}

type emptyScanResult struct {
	Candidates        []string
	Errs              []PathError
	VisitedDirs       int
	Cancelled         bool
	ErrorLimitHit     bool
	CandidateLimitHit bool
}

type emptySubtreeResult struct {
	empty      bool
	candidates []string
}

type emptyScanCollector struct {
	opts              EmptyFolderScanOptions
	errs              []PathError
	visitedDirs       int
	cancelled         bool
	errorLimitHit     bool
	candidateLimitHit bool
}

func DefaultEmptyFolderRoots() []EmptyFolderRoot {
	programData := os.Getenv("PROGRAMDATA")
	programFiles := os.Getenv("ProgramFiles")
	programFilesX86 := os.Getenv("ProgramFiles(x86)")
	localAppData := os.Getenv("LOCALAPPDATA")
	appData := os.Getenv("APPDATA")
	userProfile := os.Getenv("USERPROFILE")

	roots := []EmptyFolderRoot{
		{Label: "ProgramData", Path: programData, On: programData != ""},
		{Label: "Program Files", Path: programFiles, On: programFiles != ""},
		{Label: "Program Files (x86)", Path: programFilesX86, On: programFilesX86 != ""},
		{Label: "AppData Local", Path: localAppData, On: localAppData != ""},
		{Label: "AppData Roaming", Path: appData, On: appData != ""},
	}
	if userProfile != "" {
		localLow := filepath.Join(userProfile, "AppData", "LocalLow")
		roots = append(roots, EmptyFolderRoot{Label: "AppData LocalLow", Path: localLow, On: true})
		roots = append(roots, profileAppConfigRoots(userProfile)...)
	}
	return uniqueEmptyRoots(roots)
}

func profileAppConfigRoots(userProfile string) []EmptyFolderRoot {
	roots := make([]EmptyFolderRoot, 0)
	entries, err := os.ReadDir(userProfile)
	if err == nil {
		for _, entry := range entries {
			name := entry.Name()
			if !entry.IsDir() || !strings.HasPrefix(name, ".") || name == "." || name == ".." {
				continue
			}
			roots = append(roots, EmptyFolderRoot{
				Label: "User profile: " + name,
				Path:  filepath.Join(userProfile, name),
				On:    true,
			})
		}
	}

	for _, name := range []string{"go", "ansel"} {
		roots = append(roots, EmptyFolderRoot{
			Label: "User profile: " + name,
			Path:  filepath.Join(userProfile, name),
			On:    true,
		})
	}

	sort.SliceStable(roots, func(i, j int) bool {
		return strings.ToLower(roots[i].Label) < strings.ToLower(roots[j].Label)
	})
	return roots
}

func BuildEmptyFolderPlanWithCancel(roots []EmptyFolderRoot, shouldCancel func() bool, cb func(ProgressUpdate)) EmptyFolderPlan {
	started := time.Now()
	ctx, cancel := context.WithCancel(context.Background())
	defer cancel()

	selectedRoots := make([]EmptyFolderRoot, 0, len(roots))
	for _, root := range roots {
		root.Path = filepath.Clean(root.Path)
		if root.On && root.Path != "" {
			selectedRoots = append(selectedRoots, root)
		}
	}
	selectedRoots = dedupeEnabledEmptyRoots(selectedRoots)

	var folders []EmptyFolderCandidate
	var errs []PathError
	var visitedDirs int
	var cancelled bool
	var errorLimitHit bool
	var candidateLimitHit bool
	for i, root := range selectedRoots {
		if shouldCancel != nil && shouldCancel() {
			cancel()
		}
		if ctx.Err() != nil {
			cancelled = true
			break
		}
		if cb != nil {
			cb(ProgressUpdate{
				Phase:   "empty-scan",
				Current: i + 1,
				Total:   len(selectedRoots),
				Message: root.Label,
				Visited: visitedDirs,
			})
		}
		if !isSafeEmptyRoot(root.Path) {
			errs = append(errs, PathError{Path: root.Path, Error: "unsafe root"})
			continue
		}
		result := scanEmptyFoldersWithOptions(ctx, root.Path, EmptyFolderScanOptions{
			MaxErrors:     defaultEmptyScanMaxErrors,
			MaxCandidates: defaultEmptyScanMaxCandidates,
		}, func(visited int) {
			if shouldCancel != nil && shouldCancel() {
				cancel()
				return
			}
			if cb == nil {
				return
			}
			cb(ProgressUpdate{
				Phase:   "empty-scan",
				Current: i + 1,
				Total:   len(selectedRoots),
				Message: root.Label,
				Visited: visitedDirs + visited,
			})
		})
		visitedDirs += result.VisitedDirs
		errs = append(errs, result.Errs...)
		cancelled = cancelled || result.Cancelled
		errorLimitHit = errorLimitHit || result.ErrorLimitHit
		candidateLimitHit = candidateLimitHit || result.CandidateLimitHit
		for _, path := range result.Candidates {
			folders = append(folders, EmptyFolderCandidate{Path: path, On: true})
		}
		if result.Cancelled {
			break
		}
	}

	folders = uniqueEmptyCandidates(folders)
	sort.SliceStable(folders, func(i, j int) bool {
		return strings.ToLower(folders[i].Path) < strings.ToLower(folders[j].Path)
	})

	return EmptyFolderPlan{
		Roots:             roots,
		Folders:           folders,
		Errs:              errs,
		Selected:          len(folders),
		VisitedDirs:       visitedDirs,
		Cancelled:         cancelled,
		ErrorLimitHit:     errorLimitHit,
		CandidateLimitHit: candidateLimitHit,
		StartedAt:         started,
		FinishedAt:        time.Now(),
	}
}

func scanEmptyFoldersWithOptions(ctx context.Context, root string, opts EmptyFolderScanOptions, progress func(int)) emptyScanResult {
	opts = normalizeEmptyScanOptions(opts)
	root = filepath.Clean(root)
	info, err := os.Lstat(root)
	if err != nil {
		if os.IsNotExist(err) {
			return emptyScanResult{}
		}
		return emptyScanResult{Errs: []PathError{{Path: root, Error: err.Error()}}}
	}
	if !info.IsDir() || info.Mode()&os.ModeSymlink != 0 {
		return emptyScanResult{}
	}

	collector := &emptyScanCollector{opts: opts}
	subtree := scanEmptySubtree(ctx, root, root, collector, progress)
	candidates := collector.limitCandidates(subtree.candidates)

	sort.SliceStable(candidates, func(i, j int) bool {
		return strings.ToLower(candidates[i]) < strings.ToLower(candidates[j])
	})
	return emptyScanResult{
		Candidates:        candidates,
		Errs:              collector.errs,
		VisitedDirs:       collector.visitedDirs,
		Cancelled:         collector.cancelled,
		ErrorLimitHit:     collector.errorLimitHit,
		CandidateLimitHit: collector.candidateLimitHit,
	}
}

func scanEmptySubtree(ctx context.Context, path, root string, collector *emptyScanCollector, progress func(int)) emptySubtreeResult {
	if ctx.Err() != nil {
		collector.cancelled = true
		return emptySubtreeResult{}
	}

	entries, err := os.ReadDir(path)
	collector.visitedDirs++
	if progress != nil {
		progress(collector.visitedDirs)
	}
	if err != nil {
		collector.addError(path, err)
		return emptySubtreeResult{}
	}

	empty := true
	candidates := make([]string, 0)
	for _, entry := range entries {
		child := filepath.Join(path, entry.Name())
		if entry.Type()&os.ModeSymlink != 0 {
			empty = false
			continue
		}
		if !entry.IsDir() {
			empty = false
			continue
		}

		childResult := scanEmptySubtree(ctx, child, root, collector, progress)
		if collector.cancelled {
			return emptySubtreeResult{candidates: collector.limitCandidates(candidates)}
		}
		if !childResult.empty {
			empty = false
		}
		candidates = append(candidates, childResult.candidates...)
	}
	if empty && path != root {
		return emptySubtreeResult{empty: true, candidates: []string{path}}
	}
	return emptySubtreeResult{empty: empty, candidates: collector.limitCandidates(candidates)}
}

func ExecuteEmptyFolderPlan(plan *EmptyFolderPlan, cb func(ProgressUpdate)) EmptyFolderResult {
	result := EmptyFolderResult{
		StartedAt:     time.Now(),
		TotalSelected: countSelectedEmptyFolders(plan),
	}
	total := result.TotalSelected
	current := 0
	for _, folder := range plan.Folders {
		if !folder.On {
			continue
		}
		current++
		if cb != nil {
			cb(ProgressUpdate{
				Phase:   "empty-delete",
				Current: current,
				Total:   total,
				Message: folder.Path,
			})
		}
		if !isSafeEmptyCandidate(folder.Path, plan.Roots) {
			result.Failed++
			result.Errors = append(result.Errors, PathError{Path: folder.Path, Error: "unsafe path"})
			continue
		}
		if err := recycleEmptyFolder(folder.Path); err != nil {
			result.Failed++
			result.Errors = append(result.Errors, PathError{Path: folder.Path, Error: err.Error()})
			continue
		}
		result.Removed++
	}
	result.FinishedAt = time.Now()
	result.DurationMs = result.FinishedAt.Sub(result.StartedAt).Milliseconds()
	return result
}

func recycleEmptyFolder(path string) error {
	path = filepath.Clean(path)
	empty, err := isRecursivelyEmptyDir(path)
	if err != nil {
		return err
	}
	if !empty {
		return errors.New("folder is no longer empty")
	}
	return recycleOne(path)
}

func isRecursivelyEmptyDir(path string) (bool, error) {
	info, err := os.Lstat(path)
	if err != nil {
		if os.IsNotExist(err) {
			return false, nil
		}
		return false, err
	}
	if !info.IsDir() || info.Mode()&os.ModeSymlink != 0 {
		return false, nil
	}
	collector := &emptyScanCollector{opts: normalizeEmptyScanOptions(EmptyFolderScanOptions{MaxErrors: 1})}
	result := scanEmptySubtree(context.Background(), path, path, collector, nil)
	if len(collector.errs) > 0 {
		return false, errors.New(collector.errs[0].Error)
	}
	return result.empty, nil
}

func countSelectedEmptyFolders(plan *EmptyFolderPlan) int {
	var selected int
	for _, folder := range plan.Folders {
		if folder.On {
			selected++
		}
	}
	return selected
}

func isSafeEmptyRoot(path string) bool {
	if path == "" {
		return false
	}
	path = filepath.Clean(path)
	if _, err := os.Lstat(path); err != nil {
		return os.IsNotExist(err)
	}
	return true
}

func isSafeEmptyCandidate(path string, roots []EmptyFolderRoot) bool {
	rootPaths := make([]string, 0, len(roots))
	for _, root := range roots {
		if !root.On || root.Path == "" {
			continue
		}
		rootPaths = append(rootPaths, root.Path)
	}
	return isPathUnderAnyRoot(path, rootPaths)
}

func uniqueEmptyRoots(in []EmptyFolderRoot) []EmptyFolderRoot {
	seen := map[string]struct{}{}
	out := make([]EmptyFolderRoot, 0, len(in))
	for _, root := range in {
		if root.Path == "" {
			continue
		}
		root.Path = filepath.Clean(root.Path)
		key := strings.ToLower(root.Path)
		if _, ok := seen[key]; ok {
			continue
		}
		seen[key] = struct{}{}
		out = append(out, root)
	}
	return out
}

func uniqueEmptyCandidates(in []EmptyFolderCandidate) []EmptyFolderCandidate {
	seen := map[string]struct{}{}
	out := make([]EmptyFolderCandidate, 0, len(in))
	for _, candidate := range in {
		candidate.Path = filepath.Clean(candidate.Path)
		key := strings.ToLower(candidate.Path)
		if _, ok := seen[key]; ok {
			continue
		}
		seen[key] = struct{}{}
		out = append(out, candidate)
	}
	return out
}

func normalizeEmptyScanOptions(opts EmptyFolderScanOptions) EmptyFolderScanOptions {
	if opts.MaxErrors <= 0 {
		opts.MaxErrors = defaultEmptyScanMaxErrors
	}
	if opts.MaxCandidates <= 0 {
		opts.MaxCandidates = defaultEmptyScanMaxCandidates
	}
	return opts
}

func (collector *emptyScanCollector) addError(path string, err error) {
	if len(collector.errs) >= collector.opts.MaxErrors {
		collector.errorLimitHit = true
		return
	}
	collector.errs = append(collector.errs, PathError{Path: filepath.Clean(path), Error: err.Error()})
}

func (collector *emptyScanCollector) limitCandidates(candidates []string) []string {
	if len(candidates) <= collector.opts.MaxCandidates {
		return candidates
	}
	collector.candidateLimitHit = true
	return candidates[:collector.opts.MaxCandidates]
}

func dedupeEnabledEmptyRoots(in []EmptyFolderRoot) []EmptyFolderRoot {
	roots := uniqueEmptyRoots(in)
	sort.SliceStable(roots, func(i, j int) bool {
		return len(roots[i].Path) < len(roots[j].Path)
	})
	out := make([]EmptyFolderRoot, 0, len(roots))
	for _, root := range roots {
		covered := false
		for _, parent := range out {
			if isPathUnderRoot(root.Path, parent.Path) {
				covered = true
				break
			}
		}
		if !covered {
			out = append(out, root)
		}
	}
	sort.SliceStable(out, func(i, j int) bool {
		return strings.ToLower(out[i].Path) < strings.ToLower(out[j].Path)
	})
	return out
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
