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

type uiText struct {
	AppTitle string

	TabCache   string
	TabEmpty   string
	TabHistory string

	TaskReady            string
	TaskCacheScanning    string
	TaskCacheReview      string
	TaskCacheDeleting    string
	TaskEmptyChooseRoots string
	TaskEmptyScanning    string
	TaskEmptyReview      string
	TaskEmptyDeleting    string
	TaskHistory          string
	TaskHistoryFailed    string

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
	ActionScan           string
	ActionRemove         string
	ActionRoots          string
	ActionScanAgain      string
	ActionCleanAgain     string
	ActionHistory        string
	ActionClose          string
	ActionCacheCleanup   string
	ActionRefresh        string

	LabelSearch string

	PreparingCache string
	PreparingEmpty string
	LoadingHistory string

	CacheScanStatus          string
	CacheScanCardTitle       string
	CacheScanCardSubtitle    string
	CacheSearchPlaceholder   string
	CacheSortName            string
	CacheSortLargest         string
	CacheSortSmallest        string
	CacheCategoryBrowsers    string
	CacheCategoryChat        string
	CacheCategoryDevelopment string
	CacheCategoryGaming      string
	CacheCategoryMedia       string
	CacheCategorySystem      string
	CacheCategoryCreative    string
	CacheCategoryOther       string
	TogglePreviewOnly        string
	NoMatchingCacheTargets   string
	CacheTargetsCardTitle    string
	CacheTargetsCardSubtitle string
	CacheDeleteCardTitle     string
	CacheDeleteCardSubtitle  string

	EmptyNoRootsTask         string
	EmptyNoRootsMessage      string
	EmptyRootLead            string
	EmptyRootsCardTitle      string
	EmptyRootsCardSubtitle   string
	EmptyScanStatus          string
	EmptyScanCardTitle       string
	EmptyScanCardSubtitle    string
	EmptySearchPlaceholder   string
	NoEmptyFoldersFound      string
	EmptyFoldersCardTitle    string
	EmptyFoldersCardSubtitle string
	EmptyDeleteCardTitle     string
	EmptyDeleteCardSubtitle  string

	StatusPreparing string
	NotFound        string

	DialogNothingSelectedTitle string
	DialogSelectCacheGroup     string
	DialogNoRootsTitle         string
	DialogNoRootsMessage       string
	DialogSelectEmptyFolder    string
	DialogConfirmCacheTitle    string
	DialogConfirmEmptyTitle    string
	DialogPreviewTitle         string
	DialogPreviewEmpty         string
	DialogCleanupErrors        string
	DialogEmptyErrors          string
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
	ResultEmptyDidNotRun    string
	ResultEmptyComplete     string
	ResultEmptyWithErrors   string

	HistoryNoRuns               string
	HistoryRunsTitle            string
	HistoryDetailsTitle         string
	HistoryPreviousRunsTitle    string
	HistoryPreviousRunsSubtitle string

	EmptyScanCancelledPartial string
	EmptyScanCandidateLimit   string
	EmptyScanErrorLimit       string
}

func englishText() *uiText {
	return &uiText{
		AppTitle: "win-cleaner",

		TabCache:   "Cache Cleanup",
		TabEmpty:   "Empty Folders",
		TabHistory: "History",

		TaskReady:            "Ready",
		TaskCacheScanning:    "Scanning cache locations",
		TaskCacheReview:      "Review cache cleanup targets",
		TaskCacheDeleting:    "Moving selected cache files to the Recycle Bin",
		TaskEmptyChooseRoots: "Choose roots to scan",
		TaskEmptyScanning:    "Scanning for empty folders",
		TaskEmptyReview:      "Review empty folders",
		TaskEmptyDeleting:    "Moving empty folders to the Recycle Bin",
		TaskHistory:          "Cleanup history",
		TaskHistoryFailed:    "Cleanup history unavailable",

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
		ActionScan:           "Scan",
		ActionRemove:         "Remove",
		ActionRoots:          "Roots",
		ActionScanAgain:      "Scan Again",
		ActionCleanAgain:     "Clean Again",
		ActionHistory:        "History",
		ActionClose:          "Close",
		ActionCacheCleanup:   "Cache Cleanup",
		ActionRefresh:        "Refresh",

		LabelSearch: "Search:",

		PreparingCache: "Preparing cache cleanup...",
		PreparingEmpty: "Preparing empty-folder cleanup...",
		LoadingHistory: "Loading history...",

		CacheScanStatus:          "Scanning cache locations...",
		CacheScanCardTitle:       "Scanning",
		CacheScanCardSubtitle:    "Looking for cache locations to clean",
		CacheSearchPlaceholder:   "Search apps or labels...",
		CacheSortName:            "Name",
		CacheSortLargest:         "Largest first",
		CacheSortSmallest:        "Smallest first",
		CacheCategoryBrowsers:    "Browsers",
		CacheCategoryChat:        "Chat",
		CacheCategoryDevelopment: "Development",
		CacheCategoryGaming:      "Gaming",
		CacheCategoryMedia:       "Media",
		CacheCategorySystem:      "System",
		CacheCategoryCreative:    "Creative",
		CacheCategoryOther:       "Other",
		TogglePreviewOnly:        "Preview only",
		NoMatchingCacheTargets:   "No matching cleanup targets",
		CacheTargetsCardTitle:    "Cleanup Targets",
		CacheTargetsCardSubtitle: "Apps are collapsed by default. Expand an app to review paths and scan issues.",
		CacheDeleteCardTitle:     "Cleanup in progress",
		CacheDeleteCardSubtitle:  "Moving files to the Recycle Bin",

		EmptyNoRootsTask:         "No default roots available",
		EmptyNoRootsMessage:      "No default empty-folder roots are available in this environment.",
		EmptyRootLead:            "Select roots to scan for recursively empty folders.",
		EmptyRootsCardTitle:      "Empty Folder Roots",
		EmptyRootsCardSubtitle:   "Root selection stays here so scans can be repeated without leaving the tab.",
		EmptyScanStatus:          "Scanning for empty folders...",
		EmptyScanCardTitle:       "Scanning Empty Folders",
		EmptyScanCardSubtitle:    "Searching selected roots",
		EmptySearchPlaceholder:   "Search folder paths...",
		NoEmptyFoldersFound:      "No empty folders found",
		EmptyFoldersCardTitle:    "Empty Folders",
		EmptyFoldersCardSubtitle: "Selected folders will be rechecked before removal.",
		EmptyDeleteCardTitle:     "Removing Empty Folders",
		EmptyDeleteCardSubtitle:  "Moving folders to the Recycle Bin",

		StatusPreparing: "Preparing...",
		NotFound:        "not found",

		DialogNothingSelectedTitle: "Nothing Selected",
		DialogSelectCacheGroup:     "Select at least one group to clean.",
		DialogNoRootsTitle:         "No Roots Selected",
		DialogNoRootsMessage:       "Select at least one root to scan.",
		DialogSelectEmptyFolder:    "Select at least one empty folder to remove.",
		DialogConfirmCacheTitle:    "Confirm Cleanup",
		DialogConfirmEmptyTitle:    "Confirm Empty Folder Removal",
		DialogPreviewTitle:         "Preview: what would be cleaned",
		DialogPreviewEmpty:         "Nothing selected.",
		DialogCleanupErrors:        "Cleanup errors",
		DialogEmptyErrors:          "Empty folder errors",
		DialogClose:                "Close",
		DialogCleanupDetailsTitle:  "Cleanup target details",

		ResultNoGroupDetails:    "No group details recorded.",
		ResultErrorDetails:      "View error details",
		ResultDetails:           "Details",
		ResultStatusOK:          "ok",
		ResultStatusSkipped:     "skipped",
		ResultNoMatchingPaths:   "No matching paths",
		ResultScanIssues:        "Scan issues:",
		ResultCleanupDidNotRun:  "Cleanup did not run",
		ResultCleanupComplete:   "Cleanup complete",
		ResultCleanupWithErrors: "Cleanup finished with errors",
		ResultEmptyDidNotRun:    "Empty folder cleanup did not run",
		ResultEmptyComplete:     "Empty folder cleanup complete",
		ResultEmptyWithErrors:   "Empty folder cleanup finished with errors",

		HistoryNoRuns:               "No cleanup history yet.",
		HistoryRunsTitle:            "Runs",
		HistoryDetailsTitle:         "Run details",
		HistoryPreviousRunsTitle:    "Previous Cleanup Runs",
		HistoryPreviousRunsSubtitle: "Unreadable stats files are counted and skipped.",

		EmptyScanCancelledPartial: "Scan cancelled; showing partial results.",
		EmptyScanCandidateLimit:   "Candidate limit reached; narrow the selected roots and scan again.",
		EmptyScanErrorLimit:       "Error detail limit reached; additional scan errors were hidden.",
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

func (t *uiText) GroupsSelected(n int) string {
	return fmt.Sprintf("%d groups selected", n)
}

func (t *uiText) RootsSelected(n int) string {
	return fmt.Sprintf("%d roots selected", n)
}

func (t *uiText) FoldersSelected(n int) string {
	return fmt.Sprintf("%d folders selected", n)
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

func (t *uiText) EmptyScanProgress(u cleaner.ProgressUpdate) string {
	return fmt.Sprintf("Scanning (%d/%d, %d folders): %s", u.Current, u.Total, u.Visited, u.Message)
}

func (t *uiText) EmptyScanTaskProgress(u cleaner.ProgressUpdate) string {
	return fmt.Sprintf("Scanning empty folders (%d/%d)", u.Current, u.Total)
}

func (t *uiText) EmptyDeleteTaskProgress(u cleaner.ProgressUpdate) string {
	return fmt.Sprintf("Removing empty folders (%d/%d)", u.Current, u.Total)
}

func (t *uiText) ConfirmCacheCleanup(plan *cleaner.Plan) string {
	return fmt.Sprintf(
		"Move %d selected groups to the Recycle Bin?\nEstimated savings: %s\n\nFiles can be restored from the Recycle Bin.",
		plan.Selected, cleaner.HumanBytes(plan.TotalBytes),
	)
}

func (t *uiText) ConfirmEmptyRemoval(selected int) string {
	return fmt.Sprintf(
		"Move %d selected empty folders to the Recycle Bin?\n\nFolders can be restored from the Recycle Bin.",
		selected,
	)
}

func (t *uiText) CacheResultSummary(result *cleaner.ExecResult) string {
	return summaryLine(
		fmt.Sprintf("%d groups cleaned", result.TotalSelected),
		"est. "+cleaner.HumanBytes(result.TotalBytes)+" freed",
		formatDuration(result.DurationMs),
		fmt.Sprintf("%d errors", result.ErrorCount),
	)
}

func (t *uiText) EmptyResultSummary(result *cleaner.EmptyFolderResult) string {
	return summaryLine(
		fmt.Sprintf("%d removed", result.Removed),
		fmt.Sprintf("%d failed", result.Failed),
		formatDuration(result.DurationMs),
	)
}

func (t *uiText) EmptyScanFound(folders, visited int) string {
	return fmt.Sprintf("Found %d empty folders after scanning %d folders.", folders, visited)
}

func (t *uiText) MoreScanErrors(count int) string {
	return fmt.Sprintf("%d more scan errors", count)
}
