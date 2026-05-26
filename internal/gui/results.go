package gui

import (
	"fmt"
	"sort"
	"strings"
	"time"

	"fyne.io/fyne/v2"
	"fyne.io/fyne/v2/container"
	"fyne.io/fyne/v2/dialog"
	"fyne.io/fyne/v2/layout"
	"fyne.io/fyne/v2/theme"
	"fyne.io/fyne/v2/widget"

	"win-clear/internal/cleaner"
)

func showResults(result *cleaner.ExecResult, execErr error, a fyne.App, w fyne.Window, safeClose func(error)) {
	headline := "Cleanup complete"
	if execErr != nil {
		headline = "Cleanup finished with errors"
	}
	headlineLabel := widget.NewLabelWithStyle(headline, fyne.TextAlignLeading, fyne.TextStyle{Bold: true})

	statsLine := fmt.Sprintf(
		"%d groups cleaned  •  est. %s freed  •  %s",
		result.TotalSelected,
		cleaner.HumanBytes(result.TotalBytes),
		formatDuration(result.DurationMs),
	)
	statsLabel := widget.NewLabel(statsLine)

	tableHeader := container.NewGridWithColumns(
		4,
		widget.NewLabelWithStyle("App", fyne.TextAlignLeading, fyne.TextStyle{Bold: true}),
		widget.NewLabelWithStyle("Label", fyne.TextAlignLeading, fyne.TextStyle{Bold: true}),
		widget.NewLabelWithStyle("Est. size", fyne.TextAlignTrailing, fyne.TextStyle{Bold: true}),
		widget.NewLabelWithStyle("Status", fyne.TextAlignLeading, fyne.TextStyle{Bold: true}),
	)
	rows := make([]fyne.CanvasObject, 2, 2+len(result.Groups))
	rows[0] = tableHeader
	rows[1] = widget.NewSeparator()
	for i := range result.Groups {
		g := &result.Groups[i]
		statusText := "ok"
		if g.PathsFailed > 0 {
			statusText = fmt.Sprintf("%d failed", g.PathsFailed)
		} else if g.PathsAttempted == 0 {
			statusText = "skipped"
		}
		rows = append(rows, container.NewGridWithColumns(
			4,
			widget.NewLabel(g.App),
			widget.NewLabel(g.Label),
			widget.NewLabelWithStyle(cleaner.HumanBytes(g.Bytes), fyne.TextAlignTrailing, fyne.TextStyle{}),
			widget.NewLabel(statusText),
		))
	}
	tableScroll := container.NewVScroll(container.NewVBox(rows...))
	tableScroll.SetMinSize(fyne.NewSize(0, 300))

	errorSummary, errorDetails := buildErrorSummary(result)
	errBox := container.NewVBox()
	if errorSummary != "" {
		errLabel := widget.NewLabel(errorSummary)
		errLabel.Wrapping = fyne.TextWrapWord
		viewErrBtn := widget.NewButtonWithIcon("View error details", theme.WarningIcon(), func() {
			det := widget.NewLabel(errorDetails)
			det.Wrapping = fyne.TextWrapWord
			scr := container.NewVScroll(det)
			scr.SetMinSize(fyne.NewSize(720, 360))
			dialog.NewCustom("Cleanup errors", "Close", scr, w).Show()
		})
		errBox.Add(widget.NewSeparator())
		errBox.Add(errLabel)
		errBox.Add(viewErrBtn)
	}

	resultsCard := widget.NewCard(headline, statsLine, container.NewVBox(
		widget.NewSeparator(),
		container.NewPadded(container.NewVBox(
			headlineLabel,
			statsLabel,
			widget.NewSeparator(),
			tableScroll,
			errBox,
		)),
	))

	closeBtn := widget.NewButtonWithIcon("Close", theme.ConfirmIcon(), func() {
		safeClose(execErr)
	})
	closeBtn.Importance = widget.HighImportance
	runAgainBtn := widget.NewButtonWithIcon("Clean Again", theme.ViewRefreshIcon(), func() {
		showScan(a, w, cleaner.BuildRegistry(), cleaner.Options{DryRun: false}, safeClose)
	})

	footer := container.NewHBox(runAgainBtn, layout.NewSpacer(), closeBtn)

	w.SetContent(container.NewPadded(container.NewBorder(
		appHeader(),
		footer,
		nil, nil,
		resultsCard,
	)))
}

func buildErrorSummary(result *cleaner.ExecResult) (summary, details string) {
	if result == nil || result.ErrorCount == 0 {
		return "", ""
	}
	var groupNames []string
	var detailBuilder strings.Builder
	fmt.Fprintf(&detailBuilder, "Errors: %d\n\n", result.ErrorCount)
	for _, g := range result.Groups {
		if len(g.Errors) == 0 {
			continue
		}
		groupNames = append(groupNames, fmt.Sprintf("%s - %s", g.App, g.Label))
		fmt.Fprintf(&detailBuilder, "%s - %s (%d issues)\n", g.App, g.Label, len(g.Errors))
		for _, e := range g.Errors {
			fmt.Fprintf(&detailBuilder, "- %s: %s\n", e.Path, e.Error)
		}
		detailBuilder.WriteString("\n")
	}
	limit := min(3, len(groupNames))
	summary = fmt.Sprintf("Errors in %d group(s): %s", len(groupNames), strings.Join(groupNames[:limit], ", "))
	if len(groupNames) > limit {
		summary += ", ..."
	}
	return summary, detailBuilder.String()
}

func showDryRunDialog(plan cleaner.Plan, w fyne.Window) {
	if plan.Selected == 0 {
		dialog.ShowInformation("Preview", "Nothing selected.", w)
		return
	}

	headerRow := container.NewGridWithColumns(
		3,
		widget.NewLabelWithStyle("App", fyne.TextAlignLeading, fyne.TextStyle{Bold: true}),
		widget.NewLabelWithStyle("Label", fyne.TextAlignLeading, fyne.TextStyle{Bold: true}),
		widget.NewLabelWithStyle("Est. size", fyne.TextAlignTrailing, fyne.TextStyle{Bold: true}),
	)
	rows := []fyne.CanvasObject{headerRow, widget.NewSeparator()}
	for i := range plan.Groups {
		g := &plan.Groups[i]
		if !g.On {
			continue
		}
		sizeText := cleaner.HumanBytes(g.Bytes)
		if g.Bytes == 0 {
			sizeText = "not found"
		}
		rows = append(rows, container.NewGridWithColumns(
			3,
			widget.NewLabel(g.App),
			widget.NewLabel(g.Label),
			widget.NewLabelWithStyle(sizeText, fyne.TextAlignTrailing, fyne.TextStyle{}),
		))
	}

	totalLabel := widget.NewLabelWithStyle(
		fmt.Sprintf("%d groups  •  est. %s", plan.Selected, cleaner.HumanBytes(plan.TotalBytes)),
		fyne.TextAlignLeading, fyne.TextStyle{Bold: true},
	)

	scroll := container.NewVScroll(container.NewVBox(rows...))
	scroll.SetMinSize(fyne.NewSize(600, 400))
	content := container.NewBorder(nil, totalLabel, nil, nil, scroll)
	dialog.NewCustom("Preview: what would be cleaned", "Close", content, w).Show()
}

func showStatsDialog(w fyne.Window) {
	results, skipped, err := cleaner.LoadStats()
	if err != nil {
		dialog.ShowError(err, w)
		return
	}
	if len(results) == 0 {
		msg := "No cleanup history yet."
		if skipped > 0 {
			msg = fmt.Sprintf("No cleanup history yet. (%d files could not be read)", skipped)
		}
		dialog.ShowInformation("Cleanup history", msg, w)
		return
	}

	sort.Slice(results, func(i, j int) bool {
		return runTimestamp(&results[i]).After(runTimestamp(&results[j]))
	})

	list := widget.NewList(
		func() int { return len(results) },
		func() fyne.CanvasObject { return widget.NewLabel("") },
		func(i int, o fyne.CanvasObject) {
			if label, ok := o.(*widget.Label); ok {
				label.SetText(runSummaryLine(&results[i]))
			}
		},
	)
	listScroll := container.NewVScroll(list)
	listScroll.SetMinSize(fyne.NewSize(200, 420))

	detailsLabel := widget.NewLabel("")
	detailsLabel.Wrapping = fyne.TextWrapWord
	detailsScroll := container.NewVScroll(detailsLabel)
	detailsScroll.SetMinSize(fyne.NewSize(560, 420))

	updateDetails := func(idx int) {
		if idx >= 0 && idx < len(results) {
			detailsLabel.SetText(runDetailsText(&results[idx]))
		}
	}
	list.OnSelected = updateDetails
	if len(results) > 0 {
		list.Select(0)
		updateDetails(0)
	}

	meta := widget.NewLabel(statsSummaryText(results, skipped))
	meta.Wrapping = fyne.TextWrapWord

	split := container.NewGridWithColumns(2, listScroll, detailsScroll)
	content := container.NewPadded(container.NewBorder(meta, nil, nil, nil, split))
	dialog.NewCustom("Cleanup history", "Close", content, w).Show()
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

func formatDuration(ms int64) string {
	if ms <= 0 {
		return "--"
	}
	return (time.Duration(ms) * time.Millisecond).Round(time.Second).String()
}

func runSummaryLine(res *cleaner.ExecResult) string {
	if res == nil {
		return ""
	}
	return fmt.Sprintf("%s  %s", runTimestamp(res).Format("Jan 02 15:04"), cleaner.HumanBytes(res.TotalBytes))
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

func statsSummaryText(results []cleaner.ExecResult, skipped int) string {
	now := time.Now()
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
