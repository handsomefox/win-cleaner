package gui

import (
	"fmt"
	"strings"
	"sync/atomic"

	"fyne.io/fyne/v2"
	"fyne.io/fyne/v2/container"
	"fyne.io/fyne/v2/dialog"
	"fyne.io/fyne/v2/layout"
	"fyne.io/fyne/v2/theme"
	"fyne.io/fyne/v2/widget"

	"win-clear/internal/cleaner"
)

func showEmptyRootSelect(opts cleaner.Options, a fyne.App, w fyne.Window, safeClose func(error)) {
	roots := cleaner.DefaultEmptyFolderRoots()
	if len(roots) == 0 {
		dialog.ShowInformation("Empty Folders", "No default roots are available in this environment.", w)
		return
	}
	showEmptyRootSelectWithRoots(roots, opts, a, w, safeClose)
}

func showEmptyRootSelectWithRoots(roots []cleaner.EmptyFolderRoot, opts cleaner.Options, a fyne.App, w fyne.Window, safeClose func(error)) {
	selectedLabel := widget.NewLabel("")
	updateSummary := func() {
		selected := 0
		for _, root := range roots {
			if root.On {
				selected++
			}
		}
		selectedLabel.SetText(fmt.Sprintf("%d roots selected", selected))
	}

	rows := make([]fyne.CanvasObject, 0, len(roots))
	checks := make([]*widget.Check, len(roots))
	for i := range roots {
		idx := i
		root := &roots[idx]
		chk := widget.NewCheck("", func(checked bool) {
			roots[idx].On = checked
			updateSummary()
		})
		chk.Checked = root.On
		checks[i] = chk
		pathLabel := widget.NewLabel(root.Path)
		pathLabel.Truncation = fyne.TextTruncateEllipsis
		rows = append(rows, container.NewBorder(
			nil, nil,
			container.NewHBox(chk, widget.NewLabelWithStyle(root.Label, fyne.TextAlignLeading, fyne.TextStyle{Bold: true})),
			nil,
			pathLabel,
		))
	}

	selectAll := widget.NewButtonWithIcon("Select All", theme.ContentAddIcon(), func() {
		for i := range roots {
			roots[i].On = true
			checks[i].Checked = true
			checks[i].Refresh()
		}
		updateSummary()
	})
	selectNone := widget.NewButtonWithIcon("Deselect All", theme.ContentRemoveIcon(), func() {
		for i := range roots {
			roots[i].On = false
			checks[i].Checked = false
			checks[i].Refresh()
		}
		updateSummary()
	})
	scanBtn := widget.NewButtonWithIcon("Scan", theme.SearchIcon(), func() {
		selected := 0
		for _, root := range roots {
			if root.On {
				selected++
			}
		}
		if selected == 0 {
			dialog.ShowInformation("No Roots Selected", "Select at least one root to scan.", w)
			return
		}
		showEmptyScan(roots, opts, a, w, safeClose)
	})
	scanBtn.Importance = widget.HighImportance

	backBtn := widget.NewButtonWithIcon("Back", theme.NavigateBackIcon(), func() {
		showScan(a, w, cleaner.BuildRegistry(), opts, safeClose)
	})

	updateSummary()
	scroll := container.NewVScroll(container.NewVBox(rows...))
	scroll.SetMinSize(fyne.NewSize(0, 440))
	content := container.NewPadded(container.NewBorder(
		container.NewVBox(
			container.NewHBox(appHeader(), layout.NewSpacer(), selectedLabel),
			widget.NewSeparator(),
			widget.NewLabel("Select roots to scan for recursively empty folders."),
			container.NewHBox(selectAll, selectNone),
		),
		container.NewHBox(backBtn, layout.NewSpacer(), scanBtn),
		nil, nil,
		widget.NewCard("Empty Folder Roots", "", scroll),
	))
	w.SetContent(content)
}

func showEmptyScan(roots []cleaner.EmptyFolderRoot, opts cleaner.Options, a fyne.App, w fyne.Window, safeClose func(error)) {
	status := widget.NewLabel("Scanning for empty folders...")
	status.Alignment = fyne.TextAlignCenter
	progress := widget.NewProgressBar()
	var stale atomic.Bool

	content := container.NewPadded(container.NewBorder(
		appHeader(),
		container.NewHBox(layout.NewSpacer(), widget.NewButtonWithIcon("Cancel", theme.CancelIcon(), func() {
			stale.Store(true)
			showEmptyRootSelectWithRoots(roots, opts, a, w, safeClose)
		})),
		nil, nil,
		widget.NewCard("Scanning Empty Folders", "", container.NewPadded(container.NewVBox(status, progress))),
	))
	w.SetContent(content)

	go func() {
		plan := cleaner.BuildEmptyFolderPlanWithCancel(roots, stale.Load, func(u cleaner.ProgressUpdate) {
			if stale.Load() {
				return
			}
			a.Driver().DoFromGoroutine(func() {
				if stale.Load() {
					return
				}
				if u.Total > 0 {
					progress.SetValue(float64(u.Current) / float64(u.Total))
				}
				if u.Message != "" {
					status.SetText(fmt.Sprintf("Scanning (%d/%d, %d folders): %s", u.Current, u.Total, u.Visited, u.Message))
				}
			}, false)
		})
		if stale.Load() {
			return
		}
		a.Driver().DoFromGoroutine(func() {
			if stale.Load() {
				return
			}
			showEmptySelect(&plan, opts, a, w, safeClose)
		}, false)
	}()
}

func showEmptySelect(plan *cleaner.EmptyFolderPlan, opts cleaner.Options, a fyne.App, w fyne.Window, safeClose func(error)) {
	filterEntry := widget.NewEntry()
	filterEntry.SetPlaceHolder("Search folder paths...")
	selectedLabel := widget.NewLabel("")
	var listScroll *container.Scroll

	updateSummary := func() {
		plan.Selected = countSelectedEmptyFolders(plan)
		selectedLabel.SetText(selectedEmptySummary(plan))
	}

	rebuildList := func(filter string) {
		filter = strings.ToLower(strings.TrimSpace(filter))
		rows := make([]fyne.CanvasObject, 0, len(plan.Folders))
		for i := range plan.Folders {
			if filter != "" && !strings.Contains(strings.ToLower(plan.Folders[i].Path), filter) {
				continue
			}
			idx := i
			folder := &plan.Folders[idx]
			chk := widget.NewCheck("", func(checked bool) {
				plan.Folders[idx].On = checked
				updateSummary()
			})
			chk.Checked = folder.On
			pathLabel := widget.NewLabel(folder.Path)
			pathLabel.Truncation = fyne.TextTruncateEllipsis
			rows = append(rows, container.NewBorder(nil, nil, chk, nil, pathLabel))
		}
		if len(rows) == 0 {
			rows = append(rows, widget.NewLabel("No empty folders found"))
		}
		content := container.NewVBox(rows...)
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
		for i := range plan.Folders {
			plan.Folders[i].On = true
		}
		rebuildList(filterEntry.Text)
		updateSummary()
	})
	selectNone := widget.NewButtonWithIcon("Deselect All", theme.ContentRemoveIcon(), func() {
		for i := range plan.Folders {
			plan.Folders[i].On = false
		}
		rebuildList(filterEntry.Text)
		updateSummary()
	})
	rescanBtn := widget.NewButtonWithIcon("Roots", theme.NavigateBackIcon(), func() {
		showEmptyRootSelectWithRoots(plan.Roots, opts, a, w, safeClose)
	})
	removeBtn := widget.NewButtonWithIcon("Remove", theme.DeleteIcon(), func() {
		updateSummary()
		if plan.Selected == 0 {
			dialog.ShowInformation("Nothing Selected", "Select at least one empty folder to remove.", w)
			return
		}
		confirmText := fmt.Sprintf(
			"Move %d selected empty folders to the Recycle Bin?\n\nFolders can be restored from the Recycle Bin.",
			plan.Selected,
		)
		dialog.NewConfirm("Confirm Empty Folder Removal", confirmText, func(ok bool) {
			if !ok {
				return
			}
			showEmptyDelete(plan, opts, a, w, safeClose)
		}, w).Show()
	})
	removeBtn.Importance = widget.HighImportance

	errLabel := widget.NewLabel(emptyScanStatusText(plan))
	errLabel.Wrapping = fyne.TextWrapWord

	rebuildList("")
	updateSummary()
	listScroll.SetMinSize(fyne.NewSize(0, 440))
	content := container.NewPadded(container.NewBorder(
		container.NewVBox(
			container.NewHBox(appHeader(), layout.NewSpacer(), selectedLabel),
			widget.NewSeparator(),
			container.NewBorder(nil, nil, widget.NewLabelWithStyle("Filter:", fyne.TextAlignLeading, fyne.TextStyle{Bold: true}), nil, filterEntry),
			container.NewHBox(selectAll, selectNone),
			errLabel,
		),
		container.NewHBox(rescanBtn, layout.NewSpacer(), removeBtn),
		nil, nil,
		widget.NewCard("Empty Folders", "", listScroll),
	))
	w.SetContent(content)
}

func showEmptyDelete(plan *cleaner.EmptyFolderPlan, opts cleaner.Options, a fyne.App, w fyne.Window, safeClose func(error)) {
	currentFolder := widget.NewLabel("Preparing...")
	currentFolder.Truncation = fyne.TextTruncateEllipsis
	progress := widget.NewProgressBar()
	progressLabel := widget.NewLabel("0 / 0")

	content := container.NewPadded(container.NewBorder(
		appHeader(),
		nil, nil, nil,
		widget.NewCard("Removing Empty Folders", "Moving folders to the Recycle Bin", container.NewPadded(container.NewVBox(
			currentFolder,
			progress,
			progressLabel,
		))),
	))
	w.SetContent(content)

	go func() {
		result := cleaner.ExecuteEmptyFolderPlan(plan, func(u cleaner.ProgressUpdate) {
			a.Driver().DoFromGoroutine(func() {
				if u.Total > 0 {
					progress.SetValue(float64(u.Current) / float64(u.Total))
					progressLabel.SetText(fmt.Sprintf("%d / %d folders", u.Current, u.Total))
				}
				if u.Message != "" {
					currentFolder.SetText(u.Message)
				}
			}, false)
		})
		a.Driver().DoFromGoroutine(func() {
			showEmptyResults(&result, opts, a, w, safeClose)
		}, false)
	}()
}

func showEmptyResults(result *cleaner.EmptyFolderResult, opts cleaner.Options, a fyne.App, w fyne.Window, safeClose func(error)) {
	headline := "Empty folder cleanup complete"
	if result.Failed > 0 {
		headline = "Empty folder cleanup finished with errors"
	}
	summary := fmt.Sprintf("%d removed  •  %d failed  •  %s", result.Removed, result.Failed, formatDuration(result.DurationMs))
	headlineLabel := widget.NewLabelWithStyle(headline, fyne.TextAlignLeading, fyne.TextStyle{Bold: true})
	summaryLabel := widget.NewLabel(summary)

	errorBox := container.NewVBox()
	if len(result.Errors) > 0 {
		var detail strings.Builder
		for _, err := range result.Errors {
			fmt.Fprintf(&detail, "- %s: %s\n", err.Path, err.Error)
		}
		viewErrBtn := widget.NewButtonWithIcon("View error details", theme.WarningIcon(), func() {
			label := widget.NewLabel(detail.String())
			label.Wrapping = fyne.TextWrapWord
			scroll := container.NewVScroll(label)
			scroll.SetMinSize(fyne.NewSize(720, 360))
			dialog.NewCustom("Empty folder errors", "Close", scroll, w).Show()
		})
		errorBox.Add(widget.NewSeparator())
		errorBox.Add(viewErrBtn)
	}

	closeBtn := widget.NewButtonWithIcon("Close", theme.ConfirmIcon(), func() {
		safeClose(nil)
	})
	closeBtn.Importance = widget.HighImportance
	runAgainBtn := widget.NewButtonWithIcon("Scan Again", theme.ViewRefreshIcon(), func() {
		showEmptyRootSelect(opts, a, w, safeClose)
	})
	cacheBtn := widget.NewButtonWithIcon("Cache Cleanup", theme.NavigateBackIcon(), func() {
		showScan(a, w, cleaner.BuildRegistry(), opts, safeClose)
	})

	content := container.NewPadded(container.NewBorder(
		appHeader(),
		container.NewHBox(cacheBtn, runAgainBtn, layout.NewSpacer(), closeBtn),
		nil, nil,
		widget.NewCard(headline, summary, container.NewPadded(container.NewVBox(
			headlineLabel,
			summaryLabel,
			errorBox,
		))),
	))
	w.SetContent(content)
}

func emptyScanErrorText(errs []cleaner.PathError) string {
	if len(errs) == 0 {
		return ""
	}
	limit := min(3, len(errs))
	parts := make([]string, 0, limit)
	for i := range limit {
		parts = append(parts, fmt.Sprintf("%s: %s", errs[i].Path, errs[i].Error))
	}
	if len(errs) > limit {
		parts = append(parts, fmt.Sprintf("%d more scan errors", len(errs)-limit))
	}
	return "Scan issues: " + strings.Join(parts, "; ")
}

func emptyScanStatusText(plan *cleaner.EmptyFolderPlan) string {
	parts := make([]string, 0, 4)
	if plan.Cancelled {
		parts = append(parts, "Scan cancelled; showing partial results.")
	} else {
		parts = append(parts, fmt.Sprintf("Found %d empty folders after scanning %d folders.", len(plan.Folders), plan.VisitedDirs))
	}
	if plan.CandidateLimitHit {
		parts = append(parts, "Candidate limit reached; narrow the selected roots and scan again.")
	}
	if plan.ErrorLimitHit {
		parts = append(parts, "Error detail limit reached; additional scan errors were hidden.")
	}
	if errText := emptyScanErrorText(plan.Errs); errText != "" {
		parts = append(parts, errText)
	}
	return strings.Join(parts, "\n")
}
