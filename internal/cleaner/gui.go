// Package cleaner implements the scanning, planning, execution, and GUI
// for the win-cleaner Windows cache-cleaning tool.
package cleaner

import (
	"errors"
	"fmt"
	"runtime"
	"sort"
	"strings"
	"sync/atomic"
	"time"

	"fyne.io/fyne/v2"
	"fyne.io/fyne/v2/app"
	"fyne.io/fyne/v2/container"
	"fyne.io/fyne/v2/dialog"
	"fyne.io/fyne/v2/layout"
	"fyne.io/fyne/v2/theme"
	"fyne.io/fyne/v2/widget"
)

func RunGUI(reg Registry, opts Options) error {
	if runtime.GOOS != "windows" {
		return ErrGUIUnavailable
	}

	a := app.NewWithID("win-cleaner")
	w := a.NewWindow("win-cleaner")
	w.Resize(fyne.NewSize(980, 720))

	result := ErrCancelled
	var closing atomic.Bool
	safeClose := func(err error) {
		if closing.Swap(true) {
			return
		}
		result = err
		w.Close()
	}

	w.SetCloseIntercept(func() {
		safeClose(ErrCancelled)
	})

	showScan(a, w, reg, opts, safeClose)
	w.ShowAndRun()
	if errors.Is(result, ErrCancelled) {
		return ErrCancelled
	}
	return result
}

func showScan(a fyne.App, w fyne.Window, reg Registry, opts Options, safeClose func(error)) {
	status := widget.NewLabel("Scanning cache locations...")
	status.Alignment = fyne.TextAlignCenter
	progress := widget.NewProgressBar()

	scanCard := widget.NewCard("Scanning", "Looking for cache locations to clean", container.NewVBox(
		widget.NewSeparator(),
		container.NewPadded(container.NewVBox(status, progress)),
	))
	content := container.NewPadded(container.NewBorder(
		appHeader(),
		container.NewHBox(layout.NewSpacer(), widget.NewButtonWithIcon("Cancel", theme.CancelIcon(), func() {
			safeClose(ErrCancelled)
		})),
		nil, nil,
		scanCard,
	))
	w.SetContent(content)

	var closing atomic.Bool
	go func() {
		plan, err := BuildPlanWithProgress(reg, func(u ProgressUpdate) {
			if closing.Load() {
				return
			}
			a.Driver().DoFromGoroutine(func() {
				if u.Total > 0 {
					progress.SetValue(float64(u.Current) / float64(u.Total))
				}
				if u.Message != "" {
					status.SetText(fmt.Sprintf("Scanning (%d/%d): %s", u.Current, u.Total, u.Message))
				}
			}, false)
		})
		if closing.Load() {
			return
		}
		a.Driver().DoFromGoroutine(func() {
			if err != nil {
				dialog.ShowError(err, w)
				safeClose(err)
				return
			}
			showSelect(&plan, opts, a, w, safeClose)
		}, false)
	}()
}

func showSelect(plan *Plan, opts Options, a fyne.App, w fyne.Window, safeClose func(error)) {
	filterEntry := widget.NewEntry()
	filterEntry.SetPlaceHolder("Search apps or labels...")

	dryRun := opts.DryRun

	selectedLabel := widget.NewLabel("")
	savingsLabel := widget.NewLabelWithStyle("", fyne.TextAlignTrailing, fyne.TextStyle{Bold: true})

	updateSummary := func() {
		recomputeTotals(plan)
		selectedLabel.SetText(fmt.Sprintf("%d groups selected", plan.Selected))
		if plan.TotalBytes > 0 {
			savingsLabel.SetText("Est. savings: " + HumanBytes(plan.TotalBytes))
		} else {
			savingsLabel.SetText("")
		}
	}

	expanded := map[string]bool{} // collapsed by default
	sortMode := "name"            // "name" | "size-desc" | "size-asc"
	var listScroll *container.Scroll

	setAllExpanded := func(state bool) {
		for _, ag := range plan.ByApp() {
			expanded[ag.App] = state
		}
	}

	rebuildList := func(filter string) {
		filter = strings.ToLower(strings.TrimSpace(filter))
		appGroups := plan.ByApp()

		// Apply sort to app groups
		switch sortMode {
		case "size-desc":
			sort.SliceStable(appGroups, func(i, j int) bool {
				return appGroups[i].Bytes > appGroups[j].Bytes
			})
		case "size-asc":
			sort.SliceStable(appGroups, func(i, j int) bool {
				return appGroups[i].Bytes < appGroups[j].Bytes
			})
		}

		sections := make([]fyne.CanvasObject, 0, len(appGroups)*4)
		for _, ag := range appGroups {
			// Apply filter
			var visible []*Group
			for _, g := range ag.Items {
				if filter == "" || strings.Contains(strings.ToLower(g.App+" "+g.Label), filter) {
					visible = append(visible, g)
				}
			}
			if len(visible) == 0 {
				continue
			}

			// Largest items first; 0-byte (not found) sink to bottom
			sort.SliceStable(visible, func(i, j int) bool {
				return visible[i].Bytes > visible[j].Bytes
			})

			// Build item rows
			itemChecks := make([]*widget.Check, len(visible))
			itemRows := make([]fyne.CanvasObject, 0, len(visible))
			for i, g := range visible {
				grp := g
				sizeText := HumanBytes(grp.Bytes)
				if grp.Bytes == 0 {
					sizeText = "not found"
				}
				chk := widget.NewCheck("", nil)
				chk.Checked = grp.On
				itemChecks[i] = chk

				sizeLabel := widget.NewLabelWithStyle(sizeText, fyne.TextAlignTrailing, fyne.TextStyle{})
				if grp.Bytes == 0 {
					sizeLabel.TextStyle = fyne.TextStyle{Italic: true}
				}

				itemRows = append(itemRows, container.NewHBox(
					widget.NewLabel("      "),
					chk,
					widget.NewLabel(grp.Label),
					layout.NewSpacer(),
					sizeLabel,
				))
			}

			// App header totals
			var appTotalBytes uint64
			for _, g := range visible {
				appTotalBytes += g.Bytes
			}
			appSizeText := HumanBytes(appTotalBytes)

			// App-level checkbox
			appCheck := widget.NewCheck("", nil)
			onCount := 0
			for _, g := range visible {
				if g.On {
					onCount++
				}
			}
			appCheck.Checked = onCount == len(visible)

			for i, g := range visible {
				grp := g
				chk := itemChecks[i]
				chk.OnChanged = func(checked bool) {
					grp.On = checked
					on := 0
					for _, vg := range visible {
						if vg.On {
							on++
						}
					}
					appCheck.Checked = on == len(visible)
					appCheck.Refresh()
					updateSummary()
				}
			}
			appCheck.OnChanged = func(checked bool) {
				for i, g := range visible {
					g.On = checked
					itemChecks[i].Checked = checked
					itemChecks[i].Refresh()
				}
				updateSummary()
			}

			// Items container — collapsed by default
			itemsBox := container.NewVBox(itemRows...)
			isExpanded := expanded[ag.App] || filter != ""
			if !isExpanded {
				itemsBox.Hide()
			}

			// Collapse toggle — icon-only button, stable size
			expandBtn := widget.NewButtonWithIcon("", theme.MenuExpandIcon(), nil)
			expandBtn.Importance = widget.LowImportance
			if isExpanded {
				expandBtn.SetIcon(theme.MenuDropDownIcon())
			}
			appName := ag.App
			expandBtn.OnTapped = func() {
				expanded[appName] = !expanded[appName]
				if expanded[appName] {
					itemsBox.Show()
					expandBtn.SetIcon(theme.MenuDropDownIcon())
				} else {
					itemsBox.Hide()
					expandBtn.SetIcon(theme.MenuExpandIcon())
				}
				if listScroll != nil {
					listScroll.Refresh()
				}
			}

			appNameLabel := widget.NewLabelWithStyle(ag.App, fyne.TextAlignLeading, fyne.TextStyle{Bold: true})
			appSizeLabel := widget.NewLabelWithStyle(appSizeText, fyne.TextAlignTrailing, fyne.TextStyle{})
			headerRow := container.NewHBox(expandBtn, appCheck, appNameLabel, layout.NewSpacer(), appSizeLabel)

			section := container.NewVBox(headerRow, itemsBox, widget.NewSeparator())
			sections = append(sections, section)
		}

		if len(sections) == 0 {
			sections = append(sections, widget.NewLabel("No matches"))
		}

		content := container.NewVBox(sections...)
		if listScroll == nil {
			listScroll = container.NewVScroll(content)
		} else {
			listScroll.Content = content
			listScroll.Refresh()
		}
	}

	filterEntry.OnChanged = func(s string) {
		rebuildList(s)
		updateSummary()
	}

	selectAll := widget.NewButtonWithIcon("Select All", theme.ContentAddIcon(), func() {
		for i := range plan.Groups {
			plan.Groups[i].On = true
		}
		rebuildList(filterEntry.Text)
		updateSummary()
	})
	selectNonEmpty := widget.NewButton("Select Non-Empty", func() {
		for i := range plan.Groups {
			plan.Groups[i].On = plan.Groups[i].Bytes > 0
		}
		rebuildList(filterEntry.Text)
		updateSummary()
	})
	selectNone := widget.NewButtonWithIcon("Deselect All", theme.ContentRemoveIcon(), func() {
		for i := range plan.Groups {
			plan.Groups[i].On = false
		}
		rebuildList(filterEntry.Text)
		updateSummary()
	})
	expandAll := widget.NewButton("Expand All", func() {
		setAllExpanded(true)
		rebuildList(filterEntry.Text)
	})
	collapseAll := widget.NewButton("Collapse All", func() {
		setAllExpanded(false)
		rebuildList(filterEntry.Text)
	})

	sortSelect := widget.NewSelect([]string{"Sort: Name", "Sort: Largest first", "Sort: Smallest first"}, func(s string) {
		switch s {
		case "Sort: Largest first":
			sortMode = "size-desc"
		case "Sort: Smallest first":
			sortMode = "size-asc"
		default:
			sortMode = "name"
		}
		rebuildList(filterEntry.Text)
	})
	sortSelect.SetSelected("Sort: Name")

	applyLabel := func() string {
		if dryRun {
			return "Preview"
		}
		return "Clean Up"
	}

	apply := widget.NewButtonWithIcon(applyLabel(), theme.ConfirmIcon(), func() {
		updateSummary()
		if plan.Selected == 0 {
			dialog.ShowInformation("Nothing Selected", "Select at least one group to clean.", w)
			return
		}
		if dryRun {
			showDryRunDialog(*plan, w)
			return
		}
		confirmText := fmt.Sprintf(
			"Move %d selected groups to the Recycle Bin?\nEstimated savings: %s\n\nFiles can be restored from the Recycle Bin.",
			plan.Selected, HumanBytes(plan.TotalBytes),
		)
		dialog.NewConfirm("Confirm Cleanup", confirmText, func(ok bool) {
			if !ok {
				return
			}
			nextOpts := opts
			nextOpts.DryRun = dryRun
			showDelete(plan, nextOpts, a, w, safeClose)
		}, w).Show()
	})
	apply.Importance = widget.HighImportance

	dryRunToggle := widget.NewCheck("Preview only", func(checked bool) {
		dryRun = checked
		apply.SetText(applyLabel())
	})
	dryRunToggle.Checked = dryRun
	dryRunToggle.Refresh()

	rescanBtn := widget.NewButtonWithIcon("Rescan", theme.ViewRefreshIcon(), func() {
		showScan(a, w, BuildRegistry(), opts, safeClose)
	})

	statsBtn := widget.NewButtonWithIcon("History", theme.HistoryIcon(), func() {
		showStatsDialog(w)
	})

	topBar := container.NewHBox(
		appHeader(),
		layout.NewSpacer(),
		statsBtn,
		rescanBtn,
	)
	searchRow := container.NewBorder(nil, nil,
		widget.NewLabelWithStyle("Filter:", fyne.TextAlignLeading, fyne.TextStyle{Bold: true}),
		nil, filterEntry,
	)
	selectionRow := container.NewHBox(selectAll, selectNonEmpty, selectNone, widget.NewSeparator(), expandAll, collapseAll, layout.NewSpacer(), sortSelect)
	summaryRow := container.NewHBox(dryRunToggle, layout.NewSpacer(), selectedLabel, widget.NewLabel("  "), savingsLabel)
	footer := container.NewHBox(
		widget.NewButtonWithIcon("Cancel", theme.CancelIcon(), func() { safeClose(ErrCancelled) }),
		layout.NewSpacer(),
		apply,
	)

	rebuildList("")
	updateSummary()

	listCard := widget.NewCard("Cleanup Targets", "", listScroll)
	content := container.NewPadded(container.NewBorder(
		container.NewVBox(topBar, widget.NewSeparator(), searchRow, selectionRow, summaryRow),
		footer,
		nil, nil,
		listCard,
	))
	w.SetContent(content)
}

func showDelete(plan *Plan, opts Options, a fyne.App, w fyne.Window, safeClose func(error)) {
	currentGroup := widget.NewLabel("Preparing...")
	currentGroup.Truncation = fyne.TextTruncateEllipsis
	progress := widget.NewProgressBar()
	progressLabel := widget.NewLabel("0 / 0")

	progressCard := widget.NewCard("Cleanup in progress", "Moving files to the Recycle Bin", container.NewVBox(
		widget.NewSeparator(),
		container.NewPadded(container.NewVBox(currentGroup, progress, progressLabel)),
	))
	content := container.NewPadded(container.NewBorder(
		appHeader(),
		nil, nil, nil,
		progressCard,
	))
	w.SetContent(content)

	go func() {
		result, err := ExecuteWithResult(*plan, opts, func(u ProgressUpdate) {
			a.Driver().DoFromGoroutine(func() {
				if u.Total > 0 {
					progress.SetValue(float64(u.Current) / float64(u.Total))
					progressLabel.SetText(fmt.Sprintf("%d / %d groups", u.Current, u.Total))
				}
				if u.Message != "" {
					currentGroup.SetText(u.Message)
				}
			}, false)
		})
		if !opts.DryRun {
			if _, statErr := WriteStats(&result); statErr != nil {
				fmt.Printf("Failed to write stats: %v\n", statErr)
			}
		}
		a.Driver().DoFromGoroutine(func() {
			showResults(&result, err, a, w, safeClose)
		}, false)
	}()
}

func showResults(result *ExecResult, execErr error, a fyne.App, w fyne.Window, safeClose func(error)) {
	// Headline
	headline := "Cleanup complete"
	if execErr != nil {
		headline = "Cleanup finished with errors"
	}
	headlineLabel := widget.NewLabelWithStyle(headline, fyne.TextAlignLeading, fyne.TextStyle{Bold: true})

	statsLine := fmt.Sprintf("%d groups cleaned  •  est. %s freed  •  %s",
		result.TotalSelected,
		HumanBytes(result.TotalBytes),
		formatDuration(result.DurationMs),
	)
	statsLabel := widget.NewLabel(statsLine)

	// Per-group results table
	tableHeader := container.NewGridWithColumns(4,
		widget.NewLabelWithStyle("App", fyne.TextAlignLeading, fyne.TextStyle{Bold: true}),
		widget.NewLabelWithStyle("Label", fyne.TextAlignLeading, fyne.TextStyle{Bold: true}),
		widget.NewLabelWithStyle("Est. size", fyne.TextAlignTrailing, fyne.TextStyle{Bold: true}),
		widget.NewLabelWithStyle("Status", fyne.TextAlignLeading, fyne.TextStyle{Bold: true}),
	)
	rows := []fyne.CanvasObject{tableHeader, widget.NewSeparator()}
	for i := range result.Groups {
		g := &result.Groups[i]
		statusText := "ok"
		if g.PathsFailed > 0 {
			statusText = fmt.Sprintf("%d failed", g.PathsFailed)
		} else if g.PathsAttempted == 0 {
			statusText = "skipped"
		}
		rows = append(rows, container.NewGridWithColumns(4,
			widget.NewLabel(g.App),
			widget.NewLabel(g.Label),
			widget.NewLabelWithStyle(HumanBytes(g.Bytes), fyne.TextAlignTrailing, fyne.TextStyle{}),
			widget.NewLabel(statusText),
		))
	}
	tableScroll := container.NewVScroll(container.NewVBox(rows...))
	tableScroll.SetMinSize(fyne.NewSize(0, 300))

	// Error details (if any)
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

	// Buttons
	closeBtn := widget.NewButtonWithIcon("Close", theme.ConfirmIcon(), func() {
		safeClose(execErr)
	})
	closeBtn.Importance = widget.HighImportance
	runAgainBtn := widget.NewButtonWithIcon("Clean Again", theme.ViewRefreshIcon(), func() {
		showScan(a, w, BuildRegistry(), Options{DryRun: false}, safeClose)
	})

	footer := container.NewHBox(runAgainBtn, layout.NewSpacer(), closeBtn)

	w.SetContent(container.NewPadded(container.NewBorder(
		appHeader(),
		footer,
		nil, nil,
		resultsCard,
	)))
}

func buildErrorSummary(result *ExecResult) (summary, details string) {
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

// showDryRunDialog shows a per-group summary of what would be deleted.
func showDryRunDialog(plan Plan, w fyne.Window) {
	if plan.Selected == 0 {
		dialog.ShowInformation("Preview", "Nothing selected.", w)
		return
	}

	headerRow := container.NewGridWithColumns(3,
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
		sizeText := HumanBytes(g.Bytes)
		if g.Bytes == 0 {
			sizeText = "not found"
		}
		rows = append(rows, container.NewGridWithColumns(3,
			widget.NewLabel(g.App),
			widget.NewLabel(g.Label),
			widget.NewLabelWithStyle(sizeText, fyne.TextAlignTrailing, fyne.TextStyle{}),
		))
	}

	totalLabel := widget.NewLabelWithStyle(
		fmt.Sprintf("%d groups  •  est. %s", plan.Selected, HumanBytes(plan.TotalBytes)),
		fyne.TextAlignLeading, fyne.TextStyle{Bold: true},
	)

	scroll := container.NewVScroll(container.NewVBox(rows...))
	scroll.SetMinSize(fyne.NewSize(600, 400))
	content := container.NewBorder(nil, totalLabel, nil, nil, scroll)
	dialog.NewCustom("Preview: what would be cleaned", "Close", content, w).Show()
}

func showStatsDialog(w fyne.Window) {
	results, skipped, err := LoadStats()
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

// appHeader returns the shared top-of-window branding label.
func appHeader() *widget.Label {
	return widget.NewLabelWithStyle("win-cleaner", fyne.TextAlignLeading, fyne.TextStyle{Bold: true})
}

func runTimestamp(res *ExecResult) time.Time {
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

func runSummaryLine(res *ExecResult) string {
	if res == nil {
		return ""
	}
	return fmt.Sprintf("%s  %s", runTimestamp(res).Format("Jan 02 15:04"), HumanBytes(res.TotalBytes))
}

func runDetailsText(res *ExecResult) string {
	if res == nil {
		return ""
	}
	var b strings.Builder
	fmt.Fprintf(&b, "Run: %s\n", runTimestamp(res).Format("2006-01-02 15:04:05"))
	fmt.Fprintf(&b, "Duration: %s\n", formatDuration(res.DurationMs))
	fmt.Fprintf(&b, "Groups cleaned: %d\n", res.TotalSelected)
	fmt.Fprintf(&b, "Est. freed: %s\n", HumanBytes(res.TotalBytes))
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
		fmt.Fprintf(&b, "%s - %s  [%s  %s]\n", g.App, g.Label, HumanBytes(g.Bytes), status)
		for _, e := range g.Errors {
			fmt.Fprintf(&b, "  ! %s: %s\n", e.Path, e.Error)
		}
	}
	return b.String()
}

func statsSummaryText(results []ExecResult, skipped int) string {
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
		"Total freed: " + HumanBytes(totalAll),
		"Last 7 days: " + HumanBytes(total7),
		"Last 30 days: " + HumanBytes(total30),
	}, "\n")
}
