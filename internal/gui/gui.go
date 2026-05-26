// Package gui implements the Fyne interface for win-cleaner.
package gui

import (
	"errors"
	"fmt"
	"runtime"
	"sort"
	"strings"
	"sync/atomic"

	"fyne.io/fyne/v2"
	"fyne.io/fyne/v2/app"
	"fyne.io/fyne/v2/container"
	"fyne.io/fyne/v2/dialog"
	"fyne.io/fyne/v2/layout"
	"fyne.io/fyne/v2/theme"
	"fyne.io/fyne/v2/widget"

	"win-clear/internal/cleaner"
)

func Run(reg cleaner.Registry, opts cleaner.Options) error {
	if runtime.GOOS != "windows" {
		return cleaner.ErrGUIUnavailable
	}

	a := app.NewWithID("win-cleaner")
	w := a.NewWindow("win-cleaner")
	w.Resize(fyne.NewSize(980, 720))

	result := cleaner.ErrCancelled
	var closing atomic.Bool
	safeClose := func(err error) {
		if closing.Swap(true) {
			return
		}
		result = err
		w.Close()
	}

	w.SetCloseIntercept(func() {
		safeClose(cleaner.ErrCancelled)
	})

	showScan(a, w, reg, opts, safeClose)
	w.ShowAndRun()
	if errors.Is(result, cleaner.ErrCancelled) {
		return cleaner.ErrCancelled
	}
	return result
}

func showScan(a fyne.App, w fyne.Window, reg cleaner.Registry, opts cleaner.Options, safeClose func(error)) {
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
			safeClose(cleaner.ErrCancelled)
		})),
		nil, nil,
		scanCard,
	))
	w.SetContent(content)

	var closing atomic.Bool
	go func() {
		plan, err := cleaner.BuildPlanWithProgress(reg, func(u cleaner.ProgressUpdate) {
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

func showSelect(plan *cleaner.Plan, opts cleaner.Options, a fyne.App, w fyne.Window, safeClose func(error)) {
	filterEntry := widget.NewEntry()
	filterEntry.SetPlaceHolder("Search apps or labels...")

	dryRun := opts.DryRun

	selectedLabel := widget.NewLabel("")
	savingsLabel := widget.NewLabelWithStyle("", fyne.TextAlignTrailing, fyne.TextStyle{Bold: true})

	updateSummary := func() {
		recomputePlanTotals(plan)
		selectedLabel.SetText(fmt.Sprintf("%d groups selected", plan.Selected))
		if plan.TotalBytes > 0 {
			savingsLabel.SetText("Est. savings: " + cleaner.HumanBytes(plan.TotalBytes))
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
			var visible []*cleaner.Group
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
				sizeText := cleaner.HumanBytes(grp.Bytes)
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
			appSizeText := cleaner.HumanBytes(appTotalBytes)

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
			plan.Selected, cleaner.HumanBytes(plan.TotalBytes),
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
		showScan(a, w, cleaner.BuildRegistry(), opts, safeClose)
	})

	statsBtn := widget.NewButtonWithIcon("History", theme.HistoryIcon(), func() {
		showStatsDialog(w)
	})
	emptyFoldersBtn := widget.NewButtonWithIcon("Empty Folders", theme.FolderIcon(), func() {
		showEmptyRootSelect(opts, a, w, safeClose)
	})

	topBar := container.NewHBox(
		appHeader(),
		layout.NewSpacer(),
		emptyFoldersBtn,
		statsBtn,
		rescanBtn,
	)
	searchRow := container.NewBorder(
		nil, nil,
		widget.NewLabelWithStyle("Filter:", fyne.TextAlignLeading, fyne.TextStyle{Bold: true}),
		nil, filterEntry,
	)
	selectionRow := container.NewHBox(selectAll, selectNonEmpty, selectNone, widget.NewSeparator(), expandAll, collapseAll, layout.NewSpacer(), sortSelect)
	summaryRow := container.NewHBox(dryRunToggle, layout.NewSpacer(), selectedLabel, widget.NewLabel("  "), savingsLabel)
	footer := container.NewHBox(
		widget.NewButtonWithIcon("Cancel", theme.CancelIcon(), func() { safeClose(cleaner.ErrCancelled) }),
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

func showDelete(plan *cleaner.Plan, opts cleaner.Options, a fyne.App, w fyne.Window, safeClose func(error)) {
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
		result, err := cleaner.ExecuteWithResult(*plan, opts, func(u cleaner.ProgressUpdate) {
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
			if _, statErr := cleaner.WriteStats(&result); statErr != nil {
				fmt.Printf("Failed to write stats: %v\n", statErr)
			}
		}
		a.Driver().DoFromGoroutine(func() {
			showResults(&result, err, a, w, safeClose)
		}, false)
	}()
}
