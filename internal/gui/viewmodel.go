package gui

import (
	"fmt"
	"sort"
	"strings"
	"time"

	"win-clear/internal/cleaner"
)

type cacheCategoryView struct {
	Name   string
	Groups []cleaner.AppGroup
	Bytes  uint64
}

func summaryLine(parts ...string) string {
	nonEmpty := make([]string, 0, len(parts))
	for _, part := range parts {
		part = strings.TrimSpace(part)
		if part != "" {
			nonEmpty = append(nonEmpty, part)
		}
	}
	return strings.Join(nonEmpty, "  |  ")
}

func normalizedFilter(filter string) string {
	return strings.ToLower(strings.TrimSpace(filter))
}

func containsNormalized(value, filter string) bool {
	return strings.Contains(strings.ToLower(value), filter)
}

func removedSummary(removed int) string {
	return fmt.Sprintf("%d removed", removed)
}

func cacheSelectionSummary(texts *uiText, plan *cleaner.Plan) (selection, savings string) {
	if plan == nil {
		return texts.GroupsSelected(0), ""
	}
	recomputePlanTotals(plan)
	selection = texts.GroupsSelected(plan.Selected)
	if plan.TotalBytes > 0 {
		savings = texts.EstimatedSavings(plan.TotalBytes)
	}
	return selection, savings
}

func emptyRootSelectionSummary(texts *uiText, roots []cleaner.EmptyFolderRoot) string {
	selected := 0
	for _, root := range roots {
		if root.On {
			selected++
		}
	}
	return texts.RootsSelected(selected)
}

func recomputePlanTotals(plan *cleaner.Plan) {
	var total uint64
	var selected int
	for _, group := range plan.Groups {
		if group.On {
			selected++
			total += group.Bytes
		}
	}
	plan.TotalBytes = total
	plan.Selected = selected
}

func countSelectedEmptyFolders(plan *cleaner.EmptyFolderPlan) int {
	var selected int
	for _, folder := range plan.Folders {
		if folder.On {
			selected++
		}
	}
	return selected
}

func selectedEmptySummary(texts *uiText, plan *cleaner.EmptyFolderPlan) string {
	return texts.FoldersSelected(countSelectedEmptyFolders(plan))
}

func emptySelectionSummary(texts *uiText, plan *cleaner.EmptyFolderPlan) (selection, savings string) {
	if plan == nil {
		return texts.FoldersSelected(0), ""
	}
	return selectedEmptySummary(texts, plan), ""
}

func filteredCacheAppGroups(plan *cleaner.Plan, filter string, sortMode int) []cleaner.AppGroup {
	filter = strings.ToLower(strings.TrimSpace(filter))
	appGroups := plan.ByApp()
	switch sortMode {
	case cacheSortSizeDesc:
		sort.SliceStable(appGroups, func(i, j int) bool { return appGroups[i].Bytes > appGroups[j].Bytes })
	case cacheSortSizeAsc:
		sort.SliceStable(appGroups, func(i, j int) bool { return appGroups[i].Bytes < appGroups[j].Bytes })
	}

	filtered := make([]cleaner.AppGroup, 0, len(appGroups))
	for _, ag := range appGroups {
		visible := make([]*cleaner.Group, 0, len(ag.Items))
		for _, g := range ag.Items {
			haystack := strings.ToLower(g.App + " " + g.Label)
			if filter == "" || strings.Contains(haystack, filter) {
				visible = append(visible, g)
			}
		}
		if len(visible) == 0 {
			continue
		}
		sort.SliceStable(visible, func(i, j int) bool { return visible[i].Bytes > visible[j].Bytes })
		filtered = append(filtered, cleaner.AppGroup{App: ag.App, Items: visible, Bytes: ag.Bytes})
	}
	return filtered
}

func categorizedCacheAppGroups(texts *uiText, plan *cleaner.Plan, filter string, sortMode int) []cacheCategoryView {
	appGroups := filteredCacheAppGroups(plan, filter, sortMode)
	order := make([]string, 0)
	byName := make(map[string]*cacheCategoryView)
	for _, group := range appGroups {
		categoryName := cacheCategoryName(texts, group.App)
		category, ok := byName[categoryName]
		if !ok {
			order = append(order, categoryName)
			category = &cacheCategoryView{Name: categoryName}
			byName[categoryName] = category
		}
		category.Groups = append(category.Groups, group)
		category.Bytes += group.Bytes
	}

	categories := make([]cacheCategoryView, 0, len(order))
	for _, name := range order {
		categories = append(categories, *byName[name])
	}

	switch sortMode {
	case cacheSortSizeDesc:
		sort.SliceStable(categories, func(i, j int) bool { return categories[i].Bytes > categories[j].Bytes })
	case cacheSortSizeAsc:
		sort.SliceStable(categories, func(i, j int) bool { return categories[i].Bytes < categories[j].Bytes })
	default:
		sort.SliceStable(categories, func(i, j int) bool { return categories[i].Name < categories[j].Name })
	}
	return categories
}

func cacheCategoryName(texts *uiText, appName string) string {
	switch strings.ToLower(appName) {
	case "chrome", "edge", "firefox", "brave", "opera", "vivaldi":
		return texts.CacheCategoryBrowsers
	case "discord", "slack", "teams (classic)", "teams (new)", "telegram", "whatsapp", "zoom":
		return texts.CacheCategoryChat
	case "cargo", "go modules", "npm", "vscode":
		return texts.CacheCategoryDevelopment
	case "battle.net", "ea/origin", "epic games launcher", "gog galaxy", "steam", "ubisoft connect":
		return texts.CacheCategoryGaming
	case "spotify":
		return texts.CacheCategoryMedia
	case "amd", "crash dumps", "directx shader cache", "nvidia", "windows":
		return texts.CacheCategorySystem
	case "adobe", "battlefield 2042", "blender", "figma":
		return texts.CacheCategoryCreative
	default:
		return texts.CacheCategoryOther
	}
}

func cleanupResultSummary(texts *uiText, result *cleaner.ExecResult, execErr error) (headline, summary string) {
	if result == nil {
		return texts.ResultCleanupDidNotRun, ""
	}
	headline = texts.ResultCleanupComplete
	if execErr != nil || result.ErrorCount > 0 {
		headline = texts.ResultCleanupWithErrors
	}
	summary = texts.CacheResultSummary(result)
	return headline, summary
}

func emptyResultSummary(texts *uiText, result *cleaner.EmptyFolderResult) (headline, summary string) {
	if result == nil {
		return texts.ResultEmptyDidNotRun, ""
	}
	headline = texts.ResultEmptyComplete
	if result.Failed > 0 {
		headline = texts.ResultEmptyWithErrors
	}
	summary = texts.EmptyResultSummary(result)
	return headline, summary
}

func formatDuration(ms int64) string {
	if ms <= 0 {
		return "--"
	}
	return (time.Duration(ms) * time.Millisecond).Round(time.Second).String()
}

func emptyScanErrorText(texts *uiText, errs []cleaner.PathError) string {
	if len(errs) == 0 {
		return ""
	}
	limit := min(3, len(errs))
	parts := make([]string, 0, limit)
	for i := range limit {
		parts = append(parts, fmt.Sprintf("%s: %s", errs[i].Path, errs[i].Error))
	}
	if len(errs) > limit {
		parts = append(parts, texts.MoreScanErrors(len(errs)-limit))
	}
	return texts.ResultScanIssues + " " + strings.Join(parts, "; ")
}

func emptyScanStatusText(texts *uiText, plan *cleaner.EmptyFolderPlan) string {
	parts := make([]string, 0, 4)
	if plan.Cancelled {
		parts = append(parts, texts.EmptyScanCancelledPartial)
	} else {
		parts = append(parts, texts.EmptyScanFound(len(plan.Folders), plan.VisitedDirs))
	}
	if plan.CandidateLimitHit {
		parts = append(parts, texts.EmptyScanCandidateLimit)
	}
	if plan.ErrorLimitHit {
		parts = append(parts, texts.EmptyScanErrorLimit)
	}
	if errText := emptyScanErrorText(texts, plan.Errs); errText != "" {
		parts = append(parts, errText)
	}
	return strings.Join(parts, "\n")
}

func runTimestamp(res *cleaner.ExecResult) time.Time {
	if res == nil {
		return time.Time{}
	}
	if !res.FinishedAt.IsZero() {
		return res.FinishedAt
	}
	return res.StartedAt
}

func runDetailsText(res *cleaner.ExecResult) string {
	if res == nil {
		return ""
	}
	var b strings.Builder
	fmt.Fprintf(&b, "Run: %s\n", runTimestamp(res).Format("2006-01-02 15:04:05"))
	fmt.Fprintf(&b, "Duration: %s\n", formatDuration(res.DurationMs))
	fmt.Fprintf(&b, "Groups cleaned: %d\n", res.TotalSelected)
	fmt.Fprintf(&b, "Est. freed: %s\n", cleaner.HumanBytes(res.TotalBytes))
	fmt.Fprintf(&b, "Errors: %d\n", res.ErrorCount)
	b.WriteString("\n")

	if len(res.Groups) == 0 {
		b.WriteString("No group details recorded.\n")
		return b.String()
	}
	for _, g := range res.Groups {
		status := "ok"
		if g.PathsFailed > 0 {
			status = fmt.Sprintf("%d failed", g.PathsFailed)
		} else if g.PathsAttempted == 0 {
			status = "skipped"
		}
		fmt.Fprintf(&b, "%s - %s  [%s  %s]\n", g.App, g.Label, cleaner.HumanBytes(g.Bytes), status)
		for _, e := range g.Errors {
			fmt.Fprintf(&b, "  ! %s: %s\n", e.Path, e.Error)
		}
	}
	return b.String()
}

func statsSummaryText(_ *uiText, results []cleaner.ExecResult, skipped int, now time.Time) string {
	cutoff7 := now.AddDate(0, 0, -7)
	cutoff30 := now.AddDate(0, 0, -30)

	var totalAll, total7, total30 uint64
	for i := range results {
		res := &results[i]
		totalAll += res.TotalBytes
		ts := runTimestamp(res)
		if !ts.IsZero() {
			if ts.After(cutoff7) {
				total7 += res.TotalBytes
			}
			if ts.After(cutoff30) {
				total30 += res.TotalBytes
			}
		}
	}

	runs := fmt.Sprintf("Runs: %d", len(results))
	if skipped > 0 {
		runs = fmt.Sprintf("Runs: %d  (%d unreadable)", len(results), skipped)
	}
	return strings.Join([]string{
		runs,
		"Total freed: " + cleaner.HumanBytes(totalAll),
		"Last 7 days: " + cleaner.HumanBytes(total7),
		"Last 30 days: " + cleaner.HumanBytes(total30),
	}, "\n")
}
