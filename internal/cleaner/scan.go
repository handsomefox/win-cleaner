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
	"text/tabwriter"
)

type Options struct {
	DryRun      bool
	AssumeYes   bool
	Interactive bool
}

type Plan struct {
	Groups []Group // one per Item
	// Totals
	TotalBytes uint64
	Selected   int
}

type Group struct {
	App   string
	Label string
	Paths []string // resolved paths (globs expanded)
	On    bool     // selected for deletion
	Bytes uint64   // estimated reclaimable bytes
	Errs  []error
}

func BuildPlan(reg Registry) (Plan, error) {
	if runtime.GOOS != "windows" {
		return Plan{}, errors.New("this tool only supports Windows")
	}

	groups := make([]Group, 0, len(reg.Items))
	for _, it := range reg.Items {
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
				// Ignore not-exist to reduce noise
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
			On:    it.DefaultOn,
			Bytes: total,
			Errs:  errs,
		})
	}

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

	return Plan{
		Groups:     groups,
		TotalBytes: total,
		Selected:   selected,
	}, nil
}

func InteractiveSelect(plan *Plan) error {
	if plan == nil {
		return errors.New("nil plan")
	}
	fmt.Println("Interactive selection:")
	for {
		printSelection(plan)
		fmt.Println("Commands:")
		fmt.Println("- toggle <number>   Toggle a group on/off")
		fmt.Println("- all on            Select all")
		fmt.Println("- all off           Deselect all")
		fmt.Println("- done              Finish selection")
		fmt.Print("> ")

		var cmd string
		if _, err := fmt.Scan(&cmd); err != nil {
			return err
		}
		cmd = strings.ToLower(strings.TrimSpace(cmd))
		switch cmd {
		case "toggle":
			var idx int
			if _, err := fmt.Scan(&idx); err != nil {
				fmt.Println("Please provide a group number after 'toggle'.")
				continue
			}
			if idx < 1 || idx > len(plan.Groups) {
				fmt.Println("Invalid group number.")
				continue
			}
			plan.Groups[idx-1].On = !plan.Groups[idx-1].On
		case "all":
			var arg string
			if _, err := fmt.Scan(&arg); err != nil {
				fmt.Println("Use 'all on' or 'all off'.")
				continue
			}
			switch strings.ToLower(strings.TrimSpace(arg)) {
			case "on":
				for i := range plan.Groups {
					plan.Groups[i].On = true
				}
			case "off":
				for i := range plan.Groups {
					plan.Groups[i].On = false
				}
			default:
				fmt.Println("Use 'all on' or 'all off'.")
			}
		case "done":
			recomputeTotals(plan)
			return nil
		default:
			fmt.Println("Unknown command.")
		}
		recomputeTotals(plan)
	}
}

func printSelection(plan *Plan) {
	fmt.Println("------------------------------------------------------------")
	// Compact interactive table with row numbers:
	// [x]  #  App  Label  Size
	w := tabwriter.NewWriter(os.Stdout, 2, 4, 1, ' ', 0)
	gs := plan.Groups
	for i, g := range gs {
		sel := " "
		if g.On {
			sel = "x"
		}
		fmt.Fprintf(w, "[%s]\t%2d\t%s\t%s\t%s\n",
			sel, i+1, g.App, g.Label, HumanBytes(g.Bytes))
	}
	_ = w.Flush()
	fmt.Println("------------------------------------------------------------")
	recomputeTotals(plan)
	fmt.Printf("Selected groups: %d, Estimated savings: %s\n",
		plan.Selected, HumanBytes(plan.TotalBytes))
}

func ConfirmProceed(plan Plan, opts Options) (bool, error) {
	fmt.Printf("About to delete %d selected groups. Estimated savings: %s\n",
		plan.Selected, HumanBytes(plan.TotalBytes))
	fmt.Println("Deletion target: Recycle Bin")
	fmt.Print("Proceed? [y/N]: ")
	var ans string
	_, err := fmt.Scanln(&ans)
	if err != nil && err.Error() != "unexpected newline" {
		return false, err
	}
	ans = strings.TrimSpace(strings.ToLower(ans))
	return ans == "y" || ans == "yes", nil
}

func ReportPlan(plan Plan) string {
	var b strings.Builder
	fmt.Fprintln(&b, "Scan results (dry-run by default):")

	// Single compact table: [x]  App  Label  Size
	w := tabwriter.NewWriter(&b, 2, 4, 1, ' ', 0)

	// Sort by App then Label for stable viewing
	gs := append([]Group(nil), plan.Groups...)
	sort.SliceStable(gs, func(i, j int) bool {
		if gs[i].App == gs[j].App {
			return gs[i].Label < gs[j].Label
		}
		return gs[i].App < gs[j].App
	})

	for _, g := range gs {
		flag := " "
		if g.On {
			flag = "x"
		}
		fmt.Fprintf(w, "[%s]\t%s\t%s\t%s\n",
			flag, g.App, g.Label, HumanBytes(g.Bytes))
	}
	_ = w.Flush()

	fmt.Fprintf(&b, "Estimated savings (selected): %s\n", HumanBytes(plan.TotalBytes))
	return b.String()
}

func Execute(plan Plan, opts Options) error {
	if plan.Selected == 0 {
		fmt.Println("Nothing selected. No deletions performed.")
		return nil
	}
	var anyErr error
	for _, g := range plan.Groups {
		if !g.On {
			continue
		}
		fmt.Printf("Deleting: %s | %s\n", g.App, g.Label)
		for _, p := range g.Paths {
			if !isSafePath(p) {
				fmt.Printf("  Skipping unsafe path (guard): %s\n", p)
				continue
			}
			if _, err := os.Lstat(p); err != nil {
				// Skip silently if path no longer exists
				continue
			}
			if err := DeletePathSmart(p); err != nil {
				fmt.Printf("  Recycle Bin move failed (%v)\n", err)
			}
		}
	}
	return anyErr
}

func recomputeTotals(plan *Plan) {
	var total uint64
	var selected int
	for _, g := range plan.Groups {
		if g.On {
			selected++
			total += g.Bytes
		}
	}
	plan.TotalBytes = total
	plan.Selected = selected
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

// dirSize calculates total size. If the path doesn't exist, it returns (0, nil)
// to avoid noisy "not found" notes. It skips symlinks/reparse points.
func dirSize(p string) (uint64, error) {
	info, err := os.Lstat(p)
	if err != nil {
		if os.IsNotExist(err) {
			return 0, nil
		}
		return 0, err
	}
	if !info.IsDir() {
		return uint64(info.Size()), nil
	}

	var total uint64
	var mu sync.Mutex
	err = filepath.WalkDir(p, func(path string, d fs.DirEntry, walkErr error) error {
		if walkErr != nil {
			// Permission or path issues; continue
			return nil
		}
		// Skip symlinks/reparse points
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
				total += uint64(fi.Size())
				mu.Unlock()
			}
		}
		return nil
	})
	return total, err
}

// Conservative guard: allow under LOCALAPPDATA, APPDATA, PROGRAMDATA, and USERPROFILE,
// but never allow the root of those trees themselves.
func isSafePath(p string) bool {
	p = filepath.Clean(strings.ToLower(p))
	roots := []string{
		strings.ToLower(os.Getenv("LOCALAPPDATA")),
		strings.ToLower(os.Getenv("APPDATA")),
		strings.ToLower(os.Getenv("PROGRAMDATA")),
		strings.ToLower(os.Getenv("USERPROFILE")),
	}
	for _, r := range roots {
		r = filepath.Clean(r)
		if r == "" {
			continue
		}
		if p == r {
			return false
		}
		if strings.HasPrefix(p, r+string(filepath.Separator)) {
			return true
		}
	}
	return false
}
