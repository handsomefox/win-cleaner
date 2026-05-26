package cleaner

import (
	"errors"
	"fmt"
	"io/fs"
	"os"
	"path/filepath"
	"sort"
	"strings"
	"time"
)

type EmptyFolderRoot struct {
	Label string
	Path  string
	On    bool
}

type EmptyFolderPlan struct {
	Roots      []EmptyFolderRoot
	Folders    []EmptyFolderCandidate
	Errs       []PathError
	Selected   int
	StartedAt  time.Time
	FinishedAt time.Time
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

type emptyScanState struct {
	empty    bool
	blocked  bool
	errs     []PathError
	children []string
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

func BuildEmptyFolderPlan(roots []EmptyFolderRoot, cb func(ProgressUpdate)) EmptyFolderPlan {
	started := time.Now()
	selectedRoots := make([]EmptyFolderRoot, 0, len(roots))
	for _, root := range roots {
		root.Path = filepath.Clean(root.Path)
		if root.On && root.Path != "" {
			selectedRoots = append(selectedRoots, root)
		}
	}

	var folders []EmptyFolderCandidate
	var errs []PathError
	for i, root := range selectedRoots {
		if cb != nil {
			cb(ProgressUpdate{
				Phase:   "empty-scan",
				Current: i + 1,
				Total:   len(selectedRoots),
				Message: root.Label,
			})
		}
		if !isSafeEmptyRoot(root.Path) {
			errs = append(errs, PathError{Path: root.Path, Error: "unsafe root"})
			continue
		}
		found, rootErrs := scanEmptyFolders(root.Path)
		errs = append(errs, rootErrs...)
		for _, path := range found {
			folders = append(folders, EmptyFolderCandidate{Path: path, On: true})
		}
	}

	folders = uniqueEmptyCandidates(folders)
	sort.SliceStable(folders, func(i, j int) bool {
		return strings.ToLower(folders[i].Path) < strings.ToLower(folders[j].Path)
	})

	return EmptyFolderPlan{
		Roots:      roots,
		Folders:    folders,
		Errs:       errs,
		Selected:   len(folders),
		StartedAt:  started,
		FinishedAt: time.Now(),
	}
}

func scanEmptyFolders(root string) ([]string, []PathError) {
	root = filepath.Clean(root)
	info, err := os.Lstat(root)
	if err != nil {
		if os.IsNotExist(err) {
			return nil, nil
		}
		return nil, []PathError{{Path: root, Error: err.Error()}}
	}
	if !info.IsDir() || info.Mode()&os.ModeSymlink != 0 {
		return nil, nil
	}

	states := make(map[string]*emptyScanState)
	err = filepath.WalkDir(root, func(path string, d fs.DirEntry, walkErr error) error {
		clean := filepath.Clean(path)
		if walkErr != nil {
			state := ensureEmptyState(states, clean)
			state.blocked = true
			state.errs = append(state.errs, PathError{Path: clean, Error: walkErr.Error()})
			if d != nil && d.IsDir() {
				return filepath.SkipDir
			}
			return nil
		}
		state := ensureEmptyState(states, clean)
		if d.Type()&os.ModeSymlink != 0 {
			state.blocked = true
			parent := filepath.Dir(clean)
			ensureEmptyState(states, parent).children = append(ensureEmptyState(states, parent).children, clean)
			if d.IsDir() {
				return filepath.SkipDir
			}
			return nil
		}
		if !d.IsDir() {
			state.empty = false
			parent := filepath.Dir(clean)
			ensureEmptyState(states, parent).children = append(ensureEmptyState(states, parent).children, clean)
			return nil
		}
		state.empty = true
		if clean != root {
			parent := filepath.Dir(clean)
			ensureEmptyState(states, parent).children = append(ensureEmptyState(states, parent).children, clean)
		}
		return nil
	})
	if err != nil {
		return nil, []PathError{{Path: root, Error: err.Error()}}
	}

	paths := make([]string, 0, len(states))
	for path := range states {
		paths = append(paths, path)
	}
	sort.SliceStable(paths, func(i, j int) bool {
		return len(paths[i]) > len(paths[j])
	})

	var errs []PathError
	for _, path := range paths {
		state := states[path]
		for _, child := range state.children {
			childState := states[child]
			if childState == nil || !childState.empty || childState.blocked {
				state.empty = false
			}
		}
		if state.blocked {
			state.empty = false
		}
		errs = append(errs, state.errs...)
	}

	candidates := make([]string, 0)
	for _, path := range paths {
		if path == root {
			continue
		}
		state := states[path]
		if state == nil || !state.empty || state.blocked {
			continue
		}
		parent := states[filepath.Dir(path)]
		if parent != nil && parent.empty && !parent.blocked && filepath.Dir(path) != root {
			continue
		}
		candidates = append(candidates, path)
	}

	sort.SliceStable(candidates, func(i, j int) bool {
		return strings.ToLower(candidates[i]) < strings.ToLower(candidates[j])
	})
	return candidates, errs
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

	empty := true
	err = filepath.WalkDir(path, func(current string, d fs.DirEntry, walkErr error) error {
		if walkErr != nil {
			empty = false
			if d != nil && d.IsDir() {
				return filepath.SkipDir
			}
			return nil
		}
		if current == path {
			return nil
		}
		if d.Type()&os.ModeSymlink != 0 || !d.IsDir() {
			empty = false
			if d.IsDir() {
				return filepath.SkipDir
			}
		}
		return nil
	})
	if err != nil {
		return false, err
	}
	return empty, nil
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

func ensureEmptyState(states map[string]*emptyScanState, path string) *emptyScanState {
	path = filepath.Clean(path)
	state := states[path]
	if state == nil {
		state = &emptyScanState{empty: true}
		states[path] = state
	}
	return state
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
	path = filepath.Clean(path)
	lowPath := strings.ToLower(path)
	for _, root := range roots {
		if !root.On || root.Path == "" {
			continue
		}
		rootPath := filepath.Clean(root.Path)
		lowRoot := strings.ToLower(rootPath)
		if lowPath == lowRoot {
			return false
		}
		if strings.HasPrefix(lowPath, lowRoot+string(filepath.Separator)) {
			return true
		}
	}
	return false
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

func selectedEmptySummary(plan *EmptyFolderPlan) string {
	selected := countSelectedEmptyFolders(plan)
	return fmt.Sprintf("%d folders selected", selected)
}
