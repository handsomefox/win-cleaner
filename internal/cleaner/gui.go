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
	w.Resize(fyne.NewSize(980, 680))

	title := widget.NewLabelWithStyle("win-cleaner", fyne.TextAlignLeading, fyne.TextStyle{Bold: true})
	subtitle := widget.NewLabel("Windows cache cleaner")

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

	showScan(a, w, reg, title, subtitle, opts, safeClose)
	w.ShowAndRun()
	if errors.Is(result, ErrCancelled) {
		return ErrCancelled
	}
	return result
}

func showScan(a fyne.App, w fyne.Window, reg Registry, title, subtitle *widget.Label, opts Options, safeClose func(error)) {
	status := widget.NewLabel("Scanning cache locations...")
	progress := widget.NewProgressBar()
	progress.Min = 0
	progress.Max = 1

	scanCard := widget.NewCard("Scanning", "Looking for cache locations", container.NewVBox(status, progress))
	content := container.NewPadded(container.NewBorder(
		container.NewVBox(title, subtitle),
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
			showSelect(&plan, opts, a, w, title, subtitle, safeClose)
		}, false)
	}()
}

func showSelect(plan *Plan, opts Options, a fyne.App, w fyne.Window, title, subtitle *widget.Label, safeClose func(error)) {
	filterEntry := widget.NewEntry()
	filterEntry.SetPlaceHolder("Filter apps or labels")

	dryRun := opts.DryRun

	selectedLabel := widget.NewLabel("")
	savingsLabel := widget.NewLabel("")

	updateSummary := func() {
		recomputeTotals(plan)
		selectedLabel.SetText(fmt.Sprintf("Selected: %d", plan.Selected))
		savingsLabel.SetText("Est. savings: " + HumanBytes(plan.TotalBytes))
	}

	expanded := map[string]bool{} // collapsed by default
	var listScroll *container.Scroll

	rebuildList := func(filter string) {
		filter = strings.ToLower(strings.TrimSpace(filter))
		appGroups := plan.ByApp()

		sections := make([]fyne.CanvasObject, 0, len(appGroups)*4)
		for _, ag := range appGroups {
			// Apply filter — skip entire app section if nothing matches
			var visible []*Group
			for _, g := range ag.Items {
				if filter == "" {
					visible = append(visible, g)
					continue
				}
				hay := strings.ToLower(g.App + " " + g.Label)
				if strings.Contains(hay, filter) {
					visible = append(visible, g)
				}
			}
			if len(visible) == 0 {
				continue
			}

			// Push 0-byte items to the bottom (not found on disk)
			sort.SliceStable(visible, func(i, j int) bool {
				return visible[i].Bytes > visible[j].Bytes
			})

			// Build item rows first so the app header checkbox can reference them
			itemChecks := make([]*widget.Check, len(visible))
			itemRows := make([]fyne.CanvasObject, 0, len(visible))
			for i, g := range visible {
				grp := g
				sizeText := HumanBytes(grp.Bytes)
				if grp.Bytes == 0 {
					sizeText = "-"
				}
				chk := widget.NewCheck("", nil)
				chk.Checked = grp.On
				itemChecks[i] = chk

				itemRows = append(itemRows, container.NewHBox(
					widget.NewLabel("    "), // indent
					chk,
					widget.NewLabel(grp.Label),
					layout.NewSpacer(),
					widget.NewLabelWithStyle(sizeText, fyne.TextAlignTrailing, fyne.TextStyle{}),
				))
			}

			// App header checkbox — toggles all visible items in this group
			var appTotalBytes uint64
			for _, g := range visible {
				appTotalBytes += g.Bytes
			}
			appSizeText := HumanBytes(appTotalBytes)

			appCheck := widget.NewCheck("", nil)
			onCount := 0
			for _, g := range visible {
				if g.On {
					onCount++
				}
			}
			appCheck.Checked = onCount == len(visible)

			// Wire up item checkboxes: update group state + re-evaluate app header
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

			// Wire up app header checkbox: toggle all items
			appCheck.OnChanged = func(checked bool) {
				for i, g := range visible {
					g.On = checked
					itemChecks[i].Checked = checked
					itemChecks[i].Refresh()
				}
				updateSummary()
			}

			// Items container — hidden when collapsed
			itemsBox := container.NewVBox(itemRows...)
			isExpanded := expanded[ag.App] || filter != ""
			if !isExpanded {
				itemsBox.Hide()
			}

			expandIcon := "▶"
			if isExpanded {
				expandIcon = "▼"
			}
			expandBtn := widget.NewButton(expandIcon, nil)
			expandBtn.Importance = widget.LowImportance
			appName := ag.App // capture for closure
			expandBtn.OnTapped = func() {
				expanded[appName] = !expanded[appName]
				if expanded[appName] {
					itemsBox.Show()
					expandBtn.SetText("▼")
				} else {
					itemsBox.Hide()
					expandBtn.SetText("▶")
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
	selectNone := widget.NewButtonWithIcon("Select None", theme.ContentRemoveIcon(), func() {
		for i := range plan.Groups {
			plan.Groups[i].On = false
		}
		rebuildList(filterEntry.Text)
		updateSummary()
	})

	applyLabel := func() string {
		if dryRun {
			return "Show Dry Run"
		}
		return "Apply (Recycle Bin)"
	}

	apply := widget.NewButtonWithIcon(applyLabel(), theme.ConfirmIcon(), func() {
		updateSummary()
		if plan.Selected == 0 {
			dialog.ShowInformation("Nothing Selected", "Select at least one group to delete.", w)
			return
		}
		if dryRun {
			showDryRunDialog(*plan, w)
			return
		}
		confirmText := fmt.Sprintf("Move %d selected groups to the Recycle Bin?\nEstimated savings: %s",
			plan.Selected, HumanBytes(plan.TotalBytes))
		dialog.NewConfirm("Confirm Cleanup", confirmText, func(ok bool) {
			if !ok {
				return
			}
			nextOpts := opts
			nextOpts.DryRun = dryRun
			showDelete(plan, nextOpts, a, w, title, subtitle, safeClose)
		}, w).Show()
	})

	dryRunToggle := widget.NewCheck("Dry run", func(checked bool) {
		dryRun = checked
		apply.SetText(applyLabel())
	})
	dryRunToggle.Checked = dryRun
	dryRunToggle.Refresh()

	cancel := widget.NewButtonWithIcon("Cancel", theme.CancelIcon(), func() {
		safeClose(ErrCancelled)
	})

	statsBtn := widget.NewButton("Stats", func() {
		showStatsDialog(w)
	})

	header := container.NewVBox(
		container.NewHBox(title, layout.NewSpacer(), statsBtn, dryRunToggle),
		subtitle,
	)
	filterRow := container.NewBorder(nil, nil,
		widget.NewLabelWithStyle("Filter:", fyne.TextAlignLeading, fyne.TextStyle{Bold: true}),
		nil, filterEntry,
	)
	controls := container.NewHBox(selectAll, selectNone, layout.NewSpacer(), selectedLabel, savingsLabel)
	footer := container.NewHBox(layout.NewSpacer(), cancel, apply)

	rebuildList("")
	updateSummary()

	listCard := widget.NewCard("Cleanup targets", "", listScroll)
	content := container.NewPadded(container.NewBorder(
		container.NewVBox(header, filterRow, controls),
		footer,
		nil, nil,
		listCard,
	))
	w.SetContent(content)
}

func showDelete(plan *Plan, opts Options, a fyne.App, w fyne.Window, title, subtitle *widget.Label, safeClose func(error)) {
	status := widget.NewLabel("Deleting selected caches...")
	remainingLabel := widget.NewLabel("Remaining: --")
	progress := widget.NewProgressBar()
	progress.Min = 0
	progress.Max = 1

	progressCard := widget.NewCard("Cleanup in progress", "Moving files to Recycle Bin",
		container.NewVBox(status, remainingLabel, progress))
	content := container.NewPadded(container.NewBorder(
		container.NewVBox(title, subtitle),
		nil, nil, nil,
		progressCard,
	))
	w.SetContent(content)

	go func() {
		result, err := ExecuteWithResult(*plan, opts, func(u ProgressUpdate) {
			a.Driver().DoFromGoroutine(func() {
				if u.Total > 0 {
					progress.SetValue(float64(u.Current) / float64(u.Total))
					remainingLabel.SetText(fmt.Sprintf("Remaining: %d", u.Total-u.Current))
				}
				if u.Message != "" {
					status.SetText(fmt.Sprintf("Deleting (%d/%d): %s", u.Current, u.Total, u.Message))
				}
			}, false)
		})
		if !opts.DryRun {
			if _, statErr := WriteStats(&result); statErr != nil {
				fmt.Printf("Failed to write stats: %v\n", statErr)
			}
		}
		a.Driver().DoFromGoroutine(func() {
			finalMsg := "Cleanup complete."
			if err != nil {
				finalMsg = "Cleanup finished with errors."
			}
			doneLabel := widget.NewLabelWithStyle(finalMsg, fyne.TextAlignLeading, fyne.TextStyle{Bold: true})
			summaryLabel := widget.NewLabel(fmt.Sprintf(
				"Selected groups: %d  •  Estimated savings: %s",
				result.TotalSelected, HumanBytes(result.TotalBytes),
			))
			errorSummary, errorDetails := buildErrorSummary(&result)
			errorLabel := widget.NewLabel(errorSummary)
			detailsBtn := widget.NewButton("View error details", func() {
				details := widget.NewLabel(errorDetails)
				details.Wrapping = fyne.TextWrapWord
				scroll := container.NewVScroll(details)
				scroll.SetMinSize(fyne.NewSize(720, 360))
				dialog.NewCustom("Cleanup errors", "Close", scroll, w).Show()
			})
			if errorSummary == "" {
				errorLabel.Hide()
				detailsBtn.Hide()
			}
			closeBtn := widget.NewButtonWithIcon("Close", theme.ConfirmIcon(), func() {
				safeClose(err)
			})
			finalCard := widget.NewCard("Summary", "",
				container.NewVBox(doneLabel, summaryLabel, errorLabel, detailsBtn))
			w.SetContent(container.NewPadded(container.NewBorder(
				container.NewVBox(title, subtitle),
				container.NewHBox(layout.NewSpacer(), closeBtn),
				nil, nil,
				finalCard,
			)))
		}, false)
	}()
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
	summary = fmt.Sprintf("Errors in %d group(s).", len(groupNames))
	if limit > 0 {
		summary = fmt.Sprintf("Errors in %d group(s): %s", len(groupNames), strings.Join(groupNames[:limit], ", "))
		if len(groupNames) > limit {
			summary += ", ..."
		}
	}
	return summary, detailBuilder.String()
}

// showDryRunDialog shows a simple per-group summary of what would be deleted.
func showDryRunDialog(plan Plan, w fyne.Window) {
	if plan.Selected == 0 {
		dialog.ShowInformation("Dry Run", "Nothing selected.", w)
		return
	}

	// Build table rows: App | Label | Size
	header := container.NewGridWithColumns(3,
		widget.NewLabelWithStyle("App", fyne.TextAlignLeading, fyne.TextStyle{Bold: true}),
		widget.NewLabelWithStyle("Label", fyne.TextAlignLeading, fyne.TextStyle{Bold: true}),
		widget.NewLabelWithStyle("Est. size", fyne.TextAlignTrailing, fyne.TextStyle{Bold: true}),
	)
	rows := make([]fyne.CanvasObject, 0, plan.Selected+1)
	rows = append(rows, header, widget.NewSeparator())
	for i := range plan.Groups {
		g := &plan.Groups[i]
		if !g.On {
			continue
		}
		sizeText := HumanBytes(g.Bytes)
		if g.Bytes == 0 {
			sizeText = "—"
		}
		rows = append(rows, container.NewGridWithColumns(3,
			widget.NewLabel(g.App),
			widget.NewLabel(g.Label),
			widget.NewLabelWithStyle(sizeText, fyne.TextAlignTrailing, fyne.TextStyle{}),
		))
	}

	totalLabel := widget.NewLabelWithStyle(
		fmt.Sprintf("Total: %d groups  •  Est. savings: %s", plan.Selected, HumanBytes(plan.TotalBytes)),
		fyne.TextAlignLeading, fyne.TextStyle{Bold: true},
	)

	table := container.NewVBox(rows...)
	scroll := container.NewVScroll(table)
	scroll.SetMinSize(fyne.NewSize(600, 400))

	content := container.NewBorder(nil, totalLabel, nil, nil, scroll)
	dialog.NewCustom("Dry run — what would be deleted", "Close", content, w).Show()
}

func showStatsDialog(w fyne.Window) {
	results, skipped, err := LoadStats()
	if err != nil {
		dialog.ShowError(err, w)
		return
	}
	if len(results) == 0 {
		message := "No cleanup history yet."
		if skipped > 0 {
			message = fmt.Sprintf("No cleanup history yet. (%d files could not be read)", skipped)
		}
		dialog.ShowInformation("Cleanup history", message, w)
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
	listScroll.SetMinSize(fyne.NewSize(240, 420))

	detailsLabel := widget.NewLabel("")
	detailsLabel.Wrapping = fyne.TextWrapWord
	detailsScroll := container.NewVScroll(detailsLabel)
	detailsScroll.SetMinSize(fyne.NewSize(520, 420))

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
	return runTimestamp(res).Format("Jan 02 15:04")
}

func runDetailsText(res *ExecResult) string {
	if res == nil {
		return ""
	}
	var b strings.Builder
	fmt.Fprintf(&b, "Run: %s\n", runTimestamp(res).Format("2006-01-02 15:04:05"))
	fmt.Fprintf(&b, "Duration: %s\n", formatDuration(res.DurationMs))
	fmt.Fprintf(&b, "Selected groups: %d\n", res.TotalSelected)
	fmt.Fprintf(&b, "Estimated savings: %s\n", HumanBytes(res.TotalBytes))
	fmt.Fprintf(&b, "Errors: %d\n", res.ErrorCount)
	b.WriteString("\n")

	if len(res.Groups) == 0 {
		b.WriteString("No group details recorded.\n")
		return b.String()
	}
	for _, g := range res.Groups {
		fmt.Fprintf(&b, "%s - %s\n", g.App, g.Label)
		fmt.Fprintf(&b, "  Estimated: %s\n", HumanBytes(g.Bytes))
		fmt.Fprintf(&b, "  Paths attempted: %d\n", g.PathsAttempted)
		fmt.Fprintf(&b, "  Paths failed: %d\n", g.PathsFailed)
		if len(g.Errors) > 0 {
			b.WriteString("  Errors:\n")
			for _, e := range g.Errors {
				fmt.Fprintf(&b, "    - %s: %s\n", e.Path, e.Error)
			}
		}
		b.WriteString("\n")
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
		runs = fmt.Sprintf("Runs: %d (skipped unreadable files: %d)", len(results), skipped)
	}
	return strings.Join([]string{
		runs,
		"Total estimated cleaned: " + HumanBytes(totalAll),
		"Last 7 days: " + HumanBytes(total7),
		"Last 30 days: " + HumanBytes(total30),
	}, "\n")
}
