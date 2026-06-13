package gui

import (
	"strings"
	"testing"
	"time"

	"win-clear/internal/cleaner"
)

func TestCacheSelectionSummaryRecomputesTotals(t *testing.T) {
	texts := englishText()
	plan := cleaner.Plan{
		Groups: []cleaner.Group{
			{App: "A", Label: "one", Bytes: 1024, On: true},
			{App: "B", Label: "two", Bytes: 2048, On: false},
			{App: "C", Label: "three", Bytes: 4096, On: true},
		},
	}

	selection, savings := cacheSelectionSummary(texts, &plan)

	if selection != "2 items selected" {
		t.Fatalf("selection summary = %q, want %q", selection, "2 items selected")
	}
	if plan.Selected != 2 {
		t.Fatalf("plan.Selected = %d, want 2", plan.Selected)
	}
	if plan.TotalBytes != 5120 {
		t.Fatalf("plan.TotalBytes = %d, want 5120", plan.TotalBytes)
	}
	if !strings.Contains(savings, "Est. savings:") || !strings.Contains(savings, "5.00 KB") {
		t.Fatalf("savings summary = %q, want estimated 5.00 KB", savings)
	}
}

func TestCleanupResultSummaryReportsErrors(t *testing.T) {
	texts := englishText()
	result := cleaner.ExecResult{
		TotalSelected: 3,
		TotalBytes:    1536,
		DurationMs:    1500,
		ErrorCount:    2,
	}

	headline, summary := cleanupResultSummary(texts, &result, nil)

	if headline != "Cleanup finished with errors" {
		t.Fatalf("headline = %q, want error headline", headline)
	}
	for _, want := range []string{"3 items cleaned", "1.50 KB", "2s", "2 errors"} {
		if !strings.Contains(summary, want) {
			t.Fatalf("summary = %q, want to contain %q", summary, want)
		}
	}
}

func TestStatsSummaryTextCountsUnreadableAndWindows(t *testing.T) {
	texts := englishText()
	now := time.Date(2026, 5, 26, 12, 0, 0, 0, time.UTC)
	results := []cleaner.ExecResult{
		{FinishedAt: now.AddDate(0, 0, -1), TotalBytes: 1024},
		{FinishedAt: now.AddDate(0, 0, -20), TotalBytes: 2048},
		{FinishedAt: now.AddDate(0, 0, -40), TotalBytes: 4096},
	}

	got := statsSummaryText(texts, results, 2, now)

	for _, want := range []string{
		"Runs: 3  (2 unreadable)",
		"Total freed: 7.00 KB",
		"Last 7 days: 1.00 KB",
		"Last 30 days: 3.00 KB",
	} {
		if !strings.Contains(got, want) {
			t.Fatalf("summary = %q, want to contain %q", got, want)
		}
	}
}

func TestRunDetailsTextFormatsGroupErrors(t *testing.T) {
	res := cleaner.ExecResult{
		FinishedAt:    time.Date(2026, 5, 26, 12, 30, 0, 0, time.UTC),
		DurationMs:    1100,
		TotalSelected: 1,
		TotalBytes:    1024,
		ErrorCount:    1,
		Groups: []cleaner.GroupResult{
			{
				App:            "App",
				Label:          "Cache",
				Bytes:          1024,
				PathsAttempted: 2,
				PathsFailed:    1,
				Errors:         []cleaner.PathError{{Path: "bad", Error: "denied"}},
			},
		},
	}

	got := runDetailsText(englishText(), &res)

	for _, want := range []string{"Run: May 26, 2026 12:30:00", "Items cleaned: 1", "App - Cache", "bad: denied"} {
		if !strings.Contains(got, want) {
			t.Fatalf("details = %q, want to contain %q", got, want)
		}
	}
}

func TestFilteredCacheAppGroupsFiltersAndSorts(t *testing.T) {
	plan := cleaner.Plan{
		Groups: []cleaner.Group{
			{App: "Beta", Label: "Logs", Bytes: 100},
			{App: "Alpha", Label: "Cache", Bytes: 500},
			{App: "Alpha", Label: "Tiny", Bytes: 10},
		},
	}

	groups := filteredCacheAppGroups(&plan, "cache", cacheSortName)
	if len(groups) != 1 || groups[0].App != "Alpha" || len(groups[0].Items) != 1 || groups[0].Items[0].Label != "Cache" {
		t.Fatalf("filtered groups = %#v, want only Alpha Cache", groups)
	}

	groups = filteredCacheAppGroups(&plan, "", cacheSortSizeAsc)
	if len(groups) != 2 || groups[0].App != "Beta" || groups[1].App != "Alpha" {
		t.Fatalf("sorted groups = %#v, want Beta then Alpha by total size", groups)
	}
}

func TestCategorizedCacheAppGroups(t *testing.T) {
	texts := englishText()
	plan := cleaner.Plan{
		Groups: []cleaner.Group{
			{App: "Brave", Label: "Cache", Bytes: 500},
			{App: "Discord", Label: "Cache", Bytes: 100},
			{App: "Windows", Label: "Cache", Bytes: 300},
		},
	}

	categories := categorizedCacheAppGroups(texts, &plan, "", cacheSortSizeDesc)

	if len(categories) != 3 {
		t.Fatalf("len(categories) = %d, want 3", len(categories))
	}
	if categories[0].Name != "Browsers" || categories[0].Bytes != 500 {
		t.Fatalf("first category = %#v, want Browsers with 500 bytes", categories[0])
	}
	if categories[1].Name != "System" || categories[2].Name != "Chat" {
		t.Fatalf("categories = %#v, want size-desc category ordering", categories)
	}
}
