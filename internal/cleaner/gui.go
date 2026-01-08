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

	showScan := func() {
		status := widget.NewLabel("Scanning cache locations...")
		progress := widget.NewProgressBar()
		progress.Min = 0
		progress.Max = 1
		progress.SetValue(0)

		scanCard := widget.NewCard("Scanning", "Looking for cache locations", container.NewVBox(status, progress))
		content := container.NewPadded(container.NewBorder(
			container.NewVBox(title, subtitle),
			container.NewHBox(layout.NewSpacer(), widget.NewButtonWithIcon("Cancel", theme.CancelIcon(), func() {
				safeClose(ErrCancelled)
			})),
			nil,
			nil,
			scanCard,
		))
		w.SetContent(content)

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
				showSelect(&plan, opts, w, a, title, subtitle, safeClose)
			}, false)
		}()
	}

	showScan()
	w.ShowAndRun()
	if errors.Is(result, ErrCancelled) {
		return ErrCancelled
	}
	return result
}

func showSelect(plan *Plan, opts Options, w fyne.Window, a fyne.App, title, subtitle *widget.Label, safeClose func(error)) {
	filterEntry := widget.NewEntry()
	filterEntry.SetPlaceHolder("Filter apps or labels")

	dryRun := opts.DryRun

	selectedLabel := widget.NewLabel("")
	savingsLabel := widget.NewLabel("")

	expanded := map[int]bool{}
	var listScroll *container.Scroll

	updateSummary := func() {
		recomputeTotals(plan)
		selectedLabel.SetText(fmt.Sprintf("Selected: %d", plan.Selected))
		savingsLabel.SetText("Est. savings: " + HumanBytes(plan.TotalBytes))
	}

	rebuildList := func(filter string) {
		filter = strings.ToLower(strings.TrimSpace(filter))
		items := make([]fyne.CanvasObject, 0, len(plan.Groups))
		for i := range plan.Groups {
			g := &plan.Groups[i]
			if filter != "" {
				hay := strings.ToLower(g.App + " " + g.Label)
				if !strings.Contains(hay, filter) {
					continue
				}
			}
			idx := i
			label := fmt.Sprintf("%s - %s (%s)", g.App, g.Label, HumanBytes(g.Bytes))
			check := widget.NewCheck("", func(checked bool) {
				plan.Groups[idx].On = checked
				updateSummary()
			})
			check.Checked = g.On
			check.Refresh()

			pathsHeader := widget.NewLabelWithStyle("Planned paths:", fyne.TextAlignLeading, fyne.TextStyle{Bold: true})
			var pathsText string
			if len(g.Paths) == 0 {
				pathsText = "No paths found."
			} else {
				pathsText = strings.Join(g.Paths, "\n")
			}
			pathsLabel := widget.NewLabel(pathsText)
			pathsLabel.Wrapping = fyne.TextWrapBreak
			pathsScroll := container.NewVScroll(pathsLabel)
			pathsScroll.SetMinSize(fyne.NewSize(0, 120))
			pathsBox := container.NewVBox(pathsHeader, pathsScroll)

			expand := widget.NewButton("Show paths", nil)
			if expanded[idx] {
				pathsBox.Show()
				expand.SetText("Hide paths")
			} else {
				pathsBox.Hide()
			}
			expand.OnTapped = func() {
				expanded[idx] = !expanded[idx]
				if expanded[idx] {
					pathsBox.Show()
					expand.SetText("Hide paths")
				} else {
					pathsBox.Hide()
					expand.SetText("Show paths")
				}
			}

			header := container.NewHBox(check, widget.NewLabel(label), layout.NewSpacer(), expand)
			row := container.NewVBox(header, pathsBox, widget.NewSeparator())
			items = append(items, row)
		}
		if len(items) == 0 {
			items = append(items, widget.NewLabel("No matches"))
		}
		content := container.NewVBox(items...)
		if listScroll == nil {
			listScroll = container.NewVScroll(content)
		} else {
			listScroll.Content = content
			listScroll.Refresh()
		}
	}

	filterEntry.OnChanged = func(s string) {
		rebuildList(s)
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
			return "Close (Dry Run)"
		}
		return "Apply (Recycle Bin)"
	}

	apply := widget.NewButtonWithIcon(applyLabel(), theme.ConfirmIcon(), func() {
		updateSummary()
		if dryRun {
			safeClose(nil)
			return
		}
		if plan.Selected == 0 {
			dialog.ShowInformation("Nothing Selected", "Select at least one group to delete.", w)
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
			showDelete(plan, nextOpts, w, a, title, subtitle, safeClose)
		}, w).Show()
	})

	updateApplyLabel := func() {
		apply.SetText(applyLabel())
	}

	dryRunToggle := widget.NewCheck("Dry run", func(checked bool) {
		dryRun = checked
		updateApplyLabel()
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
	filterRow := container.NewBorder(nil, nil, widget.NewLabelWithStyle("Filter:", fyne.TextAlignLeading, fyne.TextStyle{Bold: true}), nil, filterEntry)
	controls := container.NewHBox(selectAll, selectNone, layout.NewSpacer(), selectedLabel, savingsLabel)
	footer := container.NewHBox(layout.NewSpacer(), cancel, apply)

	rebuildList("")
	updateSummary()

	listCard := widget.NewCard("Cleanup targets", "", listScroll)
	content := container.NewPadded(container.NewBorder(
		container.NewVBox(header, filterRow, controls),
		footer,
		nil,
		nil,
		listCard,
	))
	w.SetContent(content)
}

func showDelete(plan *Plan, opts Options, w fyne.Window, a fyne.App, title, subtitle *widget.Label, safeClose func(error)) {
	status := widget.NewLabel("Deleting selected caches...")
	remainingLabel := widget.NewLabel("Remaining: --")
	progress := widget.NewProgressBar()
	progress.Min = 0
	progress.Max = 1
	progress.SetValue(0)

	progressCard := widget.NewCard("Cleanup in progress", "Moving files to Recycle Bin", container.NewVBox(status, remainingLabel, progress))
	content := container.NewPadded(container.NewBorder(
		container.NewVBox(title, subtitle),
		nil,
		nil,
		nil,
		progressCard,
	))
	w.SetContent(content)

	go func() {
		result, err := ExecuteWithResult(*plan, opts, func(u ProgressUpdate) {
			a.Driver().DoFromGoroutine(func() {
				if u.Total > 0 {
					progress.SetValue(float64(u.Current) / float64(u.Total))
					remaining := u.Total - u.Current
					remainingLabel.SetText(fmt.Sprintf("Remaining: %d", remaining))
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
			summaryLabel := widget.NewLabel(fmt.Sprintf("Selected groups: %d  •  Estimated savings: %s", result.TotalSelected, HumanBytes(result.TotalBytes)))
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
			finalCard := widget.NewCard("Summary", "", container.NewVBox(doneLabel, summaryLabel, errorLabel, detailsBtn))
			w.SetContent(container.NewPadded(container.NewBorder(
				container.NewVBox(title, subtitle),
				container.NewHBox(layout.NewSpacer(), closeBtn),
				nil,
				nil,
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
	limit := 3
	if len(groupNames) < limit {
		limit = len(groupNames)
	}
	summary = fmt.Sprintf("Errors in %d group(s).", len(groupNames))
	if limit > 0 {
		summary = fmt.Sprintf("Errors in %d group(s): %s", len(groupNames), strings.Join(groupNames[:limit], ", "))
		if len(groupNames) > limit {
			summary += ", ..."
		}
	}
	return summary, detailBuilder.String()
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
		func() int {
			return len(results)
		},
		func() fyne.CanvasObject {
			return widget.NewLabel("")
		},
		func(i int, o fyne.CanvasObject) {
			label, ok := o.(*widget.Label)
			if !ok {
				return
			}
			label.SetText(runSummaryLine(&results[i]))
		},
	)

	detailsLabel := widget.NewLabel("")
	detailsLabel.Wrapping = fyne.TextWrapWord
	detailsScroll := container.NewVScroll(detailsLabel)
	detailsScroll.SetMinSize(fyne.NewSize(520, 420))

	updateDetails := func(idx int) {
		if idx < 0 || idx >= len(results) {
			return
		}
		detailsLabel.SetText(runDetailsText(&results[idx]))
	}

	list.OnSelected = updateDetails
	if len(results) > 0 {
		list.Select(0)
		updateDetails(0)
	}

	meta := widget.NewLabel(fmt.Sprintf("Runs: %d", len(results)))
	if skipped > 0 {
		meta.SetText(fmt.Sprintf("Runs: %d (skipped unreadable files: %d)", len(results), skipped))
	}

	split := container.NewHSplit(list, detailsScroll)
	split.Offset = 0.32
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
	ts := runTimestamp(res).Format("2006-01-02 15:04")
	return fmt.Sprintf("%s | Groups: %d | Est: %s | Errors: %d",
		ts, res.TotalSelected, HumanBytes(res.TotalBytes), res.ErrorCount)
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
