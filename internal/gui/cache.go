package gui

import (
	"fmt"
	"sync/atomic"

	"fyne.io/fyne/v2"
	"fyne.io/fyne/v2/container"
	"fyne.io/fyne/v2/dialog"
	"fyne.io/fyne/v2/theme"
	"fyne.io/fyne/v2/widget"

	"win-clear/internal/cleaner"
)

func (ws *workspace) showCacheScan() {
	status := widget.NewLabel(ws.texts.CacheScanStatus)
	status.Alignment = fyne.TextAlignCenter
	progress := widget.NewProgressBar()

	ws.showCache(scanPanel(
		ws.texts.CacheScanCardTitle,
		ws.texts.CacheScanCardSubtitle,
		status,
		progress,
	), &headerState{
		Task:       ws.texts.TaskCacheScanning,
		ActionText: ws.texts.ActionCancel,
		ActionIcon: theme.CancelIcon(),
		Action:     func() { ws.safeClose(cleaner.ErrCancelled) },
	})

	var stale atomic.Bool
	go func() {
		plan, err := cleaner.BuildPlanWithProgress(ws.reg, func(u cleaner.ProgressUpdate) {
			if stale.Load() {
				return
			}
			ws.app.Driver().DoFromGoroutine(func() {
				if u.Total > 0 {
					progress.SetValue(float64(u.Current) / float64(u.Total))
				}
				if u.Message != "" {
					status.SetText(ws.texts.CacheScanProgress(u))
					ws.updateCacheHeader(&headerState{Task: ws.texts.CacheScanTaskProgress(u)})
				}
			}, false)
		})
		if stale.Load() {
			return
		}
		ws.app.Driver().DoFromGoroutine(func() {
			if err != nil {
				dialog.ShowError(err, ws.window)
				ws.safeClose(err)
				return
			}
			ws.showCacheSelect(&plan)
		}, false)
	}()
}

func (ws *workspace) showCacheSelect(plan *cleaner.Plan) {
	dryRun := ws.opts.DryRun
	expanded := map[string]bool{}
	categoryExpanded := map[string]bool{}
	sortMode := cacheSortName
	var listScroll *container.Scroll
	var apply func()

	updateHeader := func(apply func()) {
		selection, savings := cacheSelectionSummary(ws.texts, plan)
		actionText := ws.texts.ActionCleanUp
		if dryRun {
			actionText = ws.texts.ActionPreview
		}
		ws.updateCacheHeader(&headerState{
			Selection:    selection,
			Savings:      savings,
			SavingsBytes: plan.TotalBytes,
			ActionText:   actionText,
			ActionIcon:   theme.ConfirmIcon(),
			Action:       apply,
		})
	}

	setAllExpanded := func(state bool) {
		for _, ag := range plan.ByApp() {
			expanded[ag.App] = state
		}
		for _, category := range categorizedCacheAppGroups(ws.texts, plan, "", sortMode) {
			categoryExpanded[category.Name] = state
		}
	}

	var rebuildList func()
	rebuildList = func() {
		categories := categorizedCacheAppGroups(ws.texts, plan, "", sortMode)
		sections := make([]fyne.CanvasObject, 0)
		for ci, category := range categories {
			appRows := make([]fyne.CanvasObject, 0, len(category.Groups))
			for ai, ag := range category.Groups {
				appRows = append(appRows, cacheAppSection(ws.texts, ag.App, ag.Items, ai%2 == 1, expanded, ws.window, func() {
					rebuildList()
					updateHeader(apply)
				}))
			}
			sections = append(sections, cacheCategorySection(ws.texts, category, ci%2 == 1, categoryExpanded, appRows, func() {
				setCategorySelected(category.Groups, !allCategoryGroupsSelected(category.Groups))
				rebuildList()
				updateHeader(apply)
			}))
		}
		if len(sections) == 0 {
			sections = append(sections, centeredStatus(ws.texts.NoMatchingCacheTargets))
		}
		content := container.NewVBox(sections...)
		if listScroll == nil {
			listScroll = container.NewVScroll(content)
		} else {
			listScroll.Content = content
			listScroll.Refresh()
		}
	}

	apply = func() {
		cacheSelectionSummary(ws.texts, plan)
		if plan.Selected == 0 {
			dialog.ShowInformation(ws.texts.DialogNothingSelectedTitle, ws.texts.DialogSelectCacheGroup, ws.window)
			return
		}
		if dryRun {
			showDryRunDialog(ws.texts, *plan, ws.window)
			return
		}
		dialog.NewConfirm(ws.texts.DialogConfirmCacheTitle, ws.texts.ConfirmCacheCleanup(plan), func(ok bool) {
			if ok {
				nextOpts := ws.opts
				nextOpts.DryRun = dryRun
				ws.showCacheDelete(plan, nextOpts)
			}
		}, ws.window).Show()
	}

	selectAll := widget.NewButtonWithIcon(ws.texts.ActionSelectAll, theme.ContentAddIcon(), func() {
		for i := range plan.Groups {
			plan.Groups[i].On = true
		}
		rebuildList()
		updateHeader(apply)
	})
	selectNonEmpty := widget.NewButtonWithIcon(ws.texts.ActionSelectNonEmpty, theme.CheckButtonCheckedIcon(), func() {
		for i := range plan.Groups {
			plan.Groups[i].On = plan.Groups[i].Bytes > 0
		}
		rebuildList()
		updateHeader(apply)
	})
	selectNone := widget.NewButtonWithIcon(ws.texts.ActionDeselectAll, theme.ContentRemoveIcon(), func() {
		for i := range plan.Groups {
			plan.Groups[i].On = false
		}
		rebuildList()
		updateHeader(apply)
	})
	expandAll := widget.NewButtonWithIcon(ws.texts.ActionExpandAll, theme.MenuDropDownIcon(), func() {
		setAllExpanded(true)
		rebuildList()
	})
	collapseAll := widget.NewButtonWithIcon(ws.texts.ActionCollapseAll, theme.MenuExpandIcon(), func() {
		setAllExpanded(false)
		rebuildList()
	})
	sortSelect := widget.NewSelect(ws.texts.CacheSortOptions(), func(s string) {
		sortMode = ws.texts.CacheSortMode(s)
		rebuildList()
	})
	sortSelect.SetSelected(ws.texts.CacheSortName)
	sortControl := container.NewGridWrap(fyne.NewSize(180, sortSelect.MinSize().Height), sortSelect)

	// Preview is a visible toggle button: highlighted (indigo) when active.
	previewBtn := widget.NewButtonWithIcon(ws.texts.TogglePreviewOnly, theme.VisibilityIcon(), nil)
	syncPreview := func() {
		if dryRun {
			previewBtn.Importance = widget.HighImportance
		} else {
			previewBtn.Importance = widget.MediumImportance
		}
		previewBtn.Refresh()
	}
	previewBtn.OnTapped = func() {
		dryRun = !dryRun
		syncPreview()
		updateHeader(apply)
	}
	syncPreview()
	rescanBtn := widget.NewButtonWithIcon(ws.texts.ActionRescan, theme.ViewRefreshIcon(), func() {
		ws.showCacheScan()
	})

	rebuildList()
	updateHeader(apply)
	listScroll.SetMinSize(fyne.NewSize(0, 480))

	// Selection actions on the left; view + run actions on the right.
	selectionGroup := toolbarRow(selectAll, selectNonEmpty, selectNone)
	viewGroup := toolbarRow(sortControl, expandAll, collapseAll, widget.NewSeparator(), previewBtn, rescanBtn)
	actionsRow := container.NewBorder(nil, nil, selectionGroup, viewGroup)
	// Toolbar + list live in one card so the controls clearly belong to the list.
	listHeader := container.NewVBox(actionsRow, widget.NewSeparator())
	ws.setCacheContent(container.NewPadded(surfaceCard(
		container.NewBorder(listHeader, nil, nil, nil, listScroll),
	)))
}

func (ws *workspace) showCacheDelete(plan *cleaner.Plan, opts cleaner.Options) {
	cacheSelectionSummary(ws.texts, plan)
	panel := newCleanupProgressPanel(
		ws.texts,
		ws.texts.CacheDeleteCardTitle,
		ws.texts.CacheDeleteCardSubtitle,
		ws.texts.ItemsCount(plan.Selected),
		cleaner.HumanBytes(plan.TotalBytes),
	)
	panel.SetProgress(0, plan.Selected, ws.texts.StatusPreparing, ws.texts.UnitItems)

	ws.showCache(panel.root, &headerState{Task: ws.texts.TaskCacheDeleting})

	go func() {
		result, err := cleaner.ExecuteWithResult(*plan, opts, func(u cleaner.ProgressUpdate) {
			ws.app.Driver().DoFromGoroutine(func() {
				if u.Total > 0 {
					panel.SetProgress(u.Current, u.Total, "", ws.texts.UnitItems)
					ws.updateCacheHeader(&headerState{Task: ws.texts.CacheDeleteTaskProgress(u)})
				}
				if u.Message != "" {
					panel.SetProgress(u.Current, u.Total, u.Message, ws.texts.UnitItems)
				}
			}, false)
		})
		if !opts.DryRun {
			if _, statErr := cleaner.WriteStats(&result); statErr != nil {
				fmt.Printf("Failed to write stats: %v\n", statErr)
			}
		}
		ws.app.Driver().DoFromGoroutine(func() {
			ws.showResults(&result, err)
		}, false)
	}()
}
