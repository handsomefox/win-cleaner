package gui

import (
	"fmt"

	"win-clear/internal/cleaner"
)

const (
	cacheSortName = iota
	cacheSortSizeDesc
	cacheSortSizeAsc
)

// appVersion is the human-facing version shown in Help ▸ About.
const appVersion = "1.0.0"

type uiText struct {
	AppTitle string

	TaskReady         string
	TaskCacheScanning string
	TaskCacheDeleting string
	TaskHistory       string
	TaskHistoryFailed string

	ActionStart          string
	ActionNone           string
	ActionCancel         string
	ActionCleanUp        string
	ActionPreview        string
	ActionSelectAll      string
	ActionSelectNonEmpty string
	ActionDeselectAll    string
	ActionExpandAll      string
	ActionCollapseAll    string
	ActionRescan         string
	ActionCleanAgain     string
	ActionHistory        string
	ActionDone           string
	ActionBack           string
	ActionRefresh        string

	PreparingCache string
	LoadingHistory string

	CacheScanStatus           string
	CacheScanCardTitle        string
	CacheScanCardSubtitle     string
	CacheSortName             string
	CacheSortLargest          string
	CacheSortSmallest         string
	CacheCategoryBrowsers     string
	CacheCategoryChat         string
	CacheCategoryDevelopment  string
	CacheCategoryGaming       string
	CacheCategoryMedia        string
	CacheCategorySystem       string
	CacheCategoryCreative     string
	CacheCategoryEmptyFolders string
	CacheCategoryOther        string
	TogglePreviewOnly         string
	NoMatchingCacheTargets    string
	CacheDeleteCardTitle      string
	CacheDeleteCardSubtitle   string

	StatusPreparing string
	NotFound        string

	// Progress panel.
	ProgressMovingTitle string
	StatSelected        string
	StatScope           string
	StatDestination     string
	RecycleBin          string
	UnitItems           string

	DialogNothingSelectedTitle string
	DialogSelectCacheGroup     string
	DialogConfirmCacheTitle    string
	DialogPreviewTitle         string
	DialogPreviewEmpty         string
	DialogCleanupErrors        string
	DialogClose                string
	DialogCleanupDetailsTitle  string

	ResultNoGroupDetails    string
	ResultErrorDetails      string
	ResultDetails           string
	ResultStatusOK          string
	ResultStatusSkipped     string
	ResultNoMatchingPaths   string
	ResultScanIssues        string
	ResultCleanupDidNotRun  string
	ResultCleanupComplete   string
	ResultCleanupWithErrors string
	FreedPrefix             string

	HistoryNoRuns               string
	HistoryPreviousRunsTitle    string
	HistoryPreviousRunsSubtitle string

	// Run details + stats labels.
	RunLabelRun          string
	RunLabelDuration     string
	RunLabelItems        string
	RunLabelFreed        string
	RunLabelErrors       string
	StatsRunsLabel       string
	StatsUnreadable      string
	StatsTotalFreedLabel string
	StatsLast7Label      string
	StatsLast30Label     string

	// Burger menu + About.
	MenuCacheCleanup  string
	MenuHistory       string
	MenuAbout         string
	MenuQuit          string
	AboutVersionLabel string
	AboutFonts        string
	AboutRepoLabel    string
	AboutRepoURL      string
}

func englishText() *uiText {
	return &uiText{
		AppTitle: "win-cleaner",

		TaskReady:         "Ready",
		TaskCacheScanning: "Scanning cache locations…",
		TaskCacheDeleting: "Moving selected cache files to the Recycle Bin",
		TaskHistory:       "Cleanup history",
		TaskHistoryFailed: "Cleanup history unavailable",

		ActionStart:          "Start",
		ActionNone:           "No action",
		ActionCancel:         "Cancel",
		ActionCleanUp:        "Clean Up",
		ActionPreview:        "Preview",
		ActionSelectAll:      "Select All",
		ActionSelectNonEmpty: "Select Non-Empty",
		ActionDeselectAll:    "Deselect All",
		ActionExpandAll:      "Expand All",
		ActionCollapseAll:    "Collapse All",
		ActionRescan:         "Rescan",
		ActionCleanAgain:     "Clean Again",
		ActionHistory:        "View History",
		ActionDone:           "Done",
		ActionBack:           "Back",
		ActionRefresh:        "Refresh",

		PreparingCache: "Preparing cache cleanup…",
		LoadingHistory: "Loading history…",

		CacheScanStatus:           "Scanning cache locations…",
		CacheScanCardTitle:        "Scanning Cache",
		CacheScanCardSubtitle:     "Looking for cache locations to clean.",
		CacheSortName:             "Name",
		CacheSortLargest:          "Largest first",
		CacheSortSmallest:         "Smallest first",
		CacheCategoryBrowsers:     "Browsers",
		CacheCategoryChat:         "Chat",
		CacheCategoryDevelopment:  "Development",
		CacheCategoryGaming:       "Gaming",
		CacheCategoryMedia:        "Media",
		CacheCategorySystem:       "System",
		CacheCategoryCreative:     "Creative",
		CacheCategoryEmptyFolders: "Empty folders",
		CacheCategoryOther:        "Other",
		TogglePreviewOnly:         "Preview only",
		NoMatchingCacheTargets:    "No matching cleanup targets.",
		CacheDeleteCardTitle:      "Cleaning Up",
		CacheDeleteCardSubtitle:   "Moving files to the Recycle Bin.",

		StatusPreparing: "Preparing…",
		NotFound:        "Not found",

		ProgressMovingTitle: "Moving selected items",
		StatSelected:        "Selected",
		StatScope:           "Scope",
		StatDestination:     "Destination",
		RecycleBin:          "Recycle Bin",
		UnitItems:           "items",

		DialogNothingSelectedTitle: "Nothing Selected",
		DialogSelectCacheGroup:     "Select at least one item to clean.",
		DialogConfirmCacheTitle:    "Confirm Cleanup",
		DialogPreviewTitle:         "Preview — what would be cleaned",
		DialogPreviewEmpty:         "Nothing selected.",
		DialogCleanupErrors:        "Cleanup Errors",
		DialogClose:                "Close",
		DialogCleanupDetailsTitle:  "Cleanup Target Details",

		ResultNoGroupDetails:    "No item details recorded.",
		ResultErrorDetails:      "View error details",
		ResultDetails:           "Details",
		ResultStatusOK:          "OK",
		ResultStatusSkipped:     "Skipped",
		ResultNoMatchingPaths:   "No matching paths",
		ResultScanIssues:        "Scan issues:",
		ResultCleanupDidNotRun:  "Cleanup did not run",
		ResultCleanupComplete:   "Cleanup complete",
		ResultCleanupWithErrors: "Cleanup finished with errors",
		FreedPrefix:             "Freed: ",

		HistoryNoRuns:               "No cleanup history yet.",
		HistoryPreviousRunsTitle:    "Previous Cleanup Runs",
		HistoryPreviousRunsSubtitle: "Unreadable stats files are counted and skipped.",

		RunLabelRun:          "Run:",
		RunLabelDuration:     "Duration:",
		RunLabelItems:        "Items cleaned:",
		RunLabelFreed:        "Est. freed:",
		RunLabelErrors:       "Errors:",
		StatsRunsLabel:       "Runs:",
		StatsUnreadable:      "unreadable",
		StatsTotalFreedLabel: "Total freed:",
		StatsLast7Label:      "Last 7 days:",
		StatsLast30Label:     "Last 30 days:",

		MenuCacheCleanup:  "Cache Cleanup",
		MenuHistory:       "History",
		MenuAbout:         "About win-cleaner",
		MenuQuit:          "Quit",
		AboutVersionLabel: "Version",
		AboutFonts:        "Inter — SIL Open Font License 1.1",
		AboutRepoLabel:    "Project page",
		AboutRepoURL:      "https://github.com/handsomefox/win-cleaner",
	}
}

func (t *uiText) CacheSortOptions() []string {
	return []string{t.CacheSortName, t.CacheSortLargest, t.CacheSortSmallest}
}

func (t *uiText) CacheSortMode(label string) int {
	switch label {
	case t.CacheSortLargest:
		return cacheSortSizeDesc
	case t.CacheSortSmallest:
		return cacheSortSizeAsc
	default:
		return cacheSortName
	}
}

// Pluralized count helpers.

func (t *uiText) ItemsCount(n int) string { return fmt.Sprintf("%d %s", n, plural(n, "item", "items")) }

func (t *uiText) AppsCount(n int) string { return fmt.Sprintf("%d %s", n, plural(n, "app", "apps")) }

func (t *uiText) RunsCount(n int) string   { return fmt.Sprintf("%d %s", n, plural(n, "run", "runs")) }
func (t *uiText) FailedCount(n int) string { return fmt.Sprintf("%d failed", n) }

func (t *uiText) IssuesCount(n int) string {
	return fmt.Sprintf("%d %s", n, plural(n, "issue", "issues"))
}

func (t *uiText) ErrorsCount(n int) string {
	return fmt.Sprintf("%d %s", n, plural(n, "error", "errors"))
}

func (t *uiText) ItemsSelected(n int) string { return t.ItemsCount(n) + " selected" }

func (t *uiText) SelectedOfCount(selected, total int) string {
	return fmt.Sprintf("%d/%d selected", selected, total)
}

func (t *uiText) DetailsWithIssues(n int) string {
	return fmt.Sprintf("%s (%s)", t.ResultDetails, t.IssuesCount(n))
}

func (t *uiText) FreedSummary(bytes uint64) string {
	return t.FreedPrefix + cleaner.HumanBytes(bytes)
}

func (t *uiText) EstimatedSavings(bytes uint64) string {
	return "Est. savings: " + cleaner.HumanBytes(bytes)
}

func (t *uiText) CacheScanProgress(u cleaner.ProgressUpdate) string {
	return fmt.Sprintf("Scanning (%d/%d): %s", u.Current, u.Total, u.Message)
}

func (t *uiText) CacheScanTaskProgress(u cleaner.ProgressUpdate) string {
	return fmt.Sprintf("Scanning cache (%d/%d)", u.Current, u.Total)
}

func (t *uiText) CacheDeleteTaskProgress(u cleaner.ProgressUpdate) string {
	return fmt.Sprintf("Cleaning cache (%d/%d)", u.Current, u.Total)
}

func (t *uiText) ConfirmCacheCleanup(plan *cleaner.Plan) string {
	return fmt.Sprintf(
		"Move %s to the Recycle Bin?\nEstimated savings: %s\n\nFiles can be restored from the Recycle Bin.",
		t.ItemsCount(plan.Selected), cleaner.HumanBytes(plan.TotalBytes),
	)
}

func (t *uiText) CacheResultSummary(result *cleaner.ExecResult) string {
	return summaryLine(
		t.ItemsCount(result.TotalSelected)+" cleaned",
		"est. "+cleaner.HumanBytes(result.TotalBytes)+" freed",
		formatDuration(result.DurationMs),
		t.ErrorsCount(result.ErrorCount),
	)
}
