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

	ws.setTabState(ws.texts.TabCache, &headerState{
		Task:       ws.texts.TaskCacheScanning,
		ActionText: ws.texts.ActionCancel,
		ActionIcon: theme.CancelIcon(),
		Action:     func() { ws.safeClose(cleaner.ErrCancelled) },
	})
	ws.setTabContent(ws.texts.TabCache, scanPanel(
		ws.texts.CacheScanCardTitle,
		ws.texts.CacheScanCardSubtitle,
		status,
		progress,
	))

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
					ws.setTabState(ws.texts.TabCache, &headerState{Task: ws.texts.CacheScanTaskProgress(u)})
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
	filterEntry := widget.NewEntry()
	filterEntry.SetPlaceHolder(ws.texts.CacheSearchPlaceholder)

	dryRun := ws.opts.DryRun
	expanded := map[string]bool{}
	categoryExpanded := map[string]bool{}
	sortMode := cacheSortSizeDesc
	var listScroll *container.Scroll
	var apply func()

	updateHeader := func(apply func()) {
		selection, savings := cacheSelectionSummary(ws.texts, plan)
		actionText := ws.texts.ActionCleanUp
		if dryRun {
			actionText = ws.texts.ActionPreview
		}
		ws.setTabState(ws.texts.TabCache, &headerState{
			Task:       ws.texts.TaskCacheReview,
			Selection:  selection,
			Savings:    savings,
			ActionText: actionText,
			ActionIcon: theme.ConfirmIcon(),
			Action:     apply,
		})
	}

	setAllExpanded := func(state bool) {
		for _, ag := range plan.ByApp() {
			expanded[ag.App] = state
		}
		for _, category := range categorizedCacheAppGroups(ws.texts, plan, filterEntry.Text, sortMode) {
			categoryExpanded[category.Name] = state
		}
	}

	var rebuildList func(string)
	rebuildList = func(filter string) {
		categories := categorizedCacheAppGroups(ws.texts, plan, filter, sortMode)
		sections := make([]fyne.CanvasObject, 0)
		for _, category := range categories {
			if _, ok := categoryExpanded[category.Name]; !ok {
				categoryExpanded[category.Name] = true
			}
			category := category
			appRows := make([]fyne.CanvasObject, 0, len(category.Groups))
			for _, ag := range category.Groups {
				appRows = append(appRows, cacheAppSection(ws.texts, ag.App, ag.Items, expanded, ws.window, func() {
					rebuildList(filterEntry.Text)
					updateHeader(apply)
				}))
			}
			sections = append(sections, cacheCategorySection(category, categoryExpanded, appRows, func() {
				setCategorySelected(category.Groups, !allCategoryGroupsSelected(category.Groups))
				rebuildList(filterEntry.Text)
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

	filterEntry.OnChanged = func(s string) {
		rebuildList(s)
		updateHeader(apply)
	}

	selectAll := widget.NewButtonWithIcon(ws.texts.ActionSelectAll, theme.ContentAddIcon(), func() {
		for i := range plan.Groups {
			plan.Groups[i].On = true
		}
		rebuildList(filterEntry.Text)
		updateHeader(apply)
	})
	selectNonEmpty := widget.NewButtonWithIcon(ws.texts.ActionSelectNonEmpty, theme.CheckButtonCheckedIcon(), func() {
		for i := range plan.Groups {
			plan.Groups[i].On = plan.Groups[i].Bytes > 0
		}
		rebuildList(filterEntry.Text)
		updateHeader(apply)
	})
	selectNone := widget.NewButtonWithIcon(ws.texts.ActionDeselectAll, theme.ContentRemoveIcon(), func() {
		for i := range plan.Groups {
			plan.Groups[i].On = false
		}
		rebuildList(filterEntry.Text)
		updateHeader(apply)
	})
	expandAll := widget.NewButtonWithIcon(ws.texts.ActionExpandAll, theme.MenuDropDownIcon(), func() {
		setAllExpanded(true)
		rebuildList(filterEntry.Text)
	})
	collapseAll := widget.NewButtonWithIcon(ws.texts.ActionCollapseAll, theme.MenuExpandIcon(), func() {
		setAllExpanded(false)
		rebuildList(filterEntry.Text)
	})
	sortSelect := widget.NewSelect(ws.texts.CacheSortOptions(), func(s string) {
		sortMode = ws.texts.CacheSortMode(s)
		rebuildList(filterEntry.Text)
	})
	sortSelect.SetSelected(ws.texts.CacheSortLargest)
	sortControl := container.NewGridWrap(fyne.NewSize(180, sortSelect.MinSize().Height), sortSelect)
	dryRunToggle := widget.NewCheck(ws.texts.TogglePreviewOnly, func(checked bool) {
		dryRun = checked
		updateHeader(apply)
	})
	dryRunToggle.Checked = dryRun
	dryRunToggle.Refresh()
	rescanBtn := widget.NewButtonWithIcon(ws.texts.ActionRescan, theme.ViewRefreshIcon(), func() {
		ws.showCacheScan()
	})

	rebuildList("")
	updateHeader(apply)
	listScroll.SetMinSize(fyne.NewSize(0, 480))

	infoBtn := widget.NewButtonWithIcon("", theme.InfoIcon(), func() {
		dialog.ShowInformation(ws.texts.CacheTargetsCardTitle, ws.texts.CacheTargetsCardSubtitle, ws.window)
	})
	infoBtn.Importance = widget.LowImportance
	searchRow := labeledEntryRow(ws.texts.LabelSearch, filterEntry)
	leftActions := toolbarRow(selectAll, selectNonEmpty, selectNone, widget.NewSeparator(), sortControl, widget.NewSeparator(), expandAll, collapseAll, infoBtn)
	rightActions := toolbarRow(dryRunToggle, rescanBtn)
	actions := container.NewBorder(nil, nil, leftActions, rightActions, nil)
	ws.setTabContent(ws.texts.TabCache, container.NewPadded(container.NewBorder(
		controlsPanel(searchRow, actions),
		nil, nil, nil,
		cacheListPanel(ws.texts, listScroll),
	)))
}

func (ws *workspace) showCacheDelete(plan *cleaner.Plan, opts cleaner.Options) {
	cacheSelectionSummary(ws.texts, plan)
	panel := newCleanupProgressPanel(
		ws.texts.CacheDeleteCardTitle,
		ws.texts.CacheDeleteCardSubtitle,
		fmt.Sprintf("%d groups", plan.Selected),
		cleaner.HumanBytes(plan.TotalBytes),
	)
	panel.SetProgress(0, plan.Selected, ws.texts.StatusPreparing, "groups")

	ws.setTabState(ws.texts.TabCache, &headerState{Task: ws.texts.TaskCacheDeleting})
	ws.setTabContent(ws.texts.TabCache, panel.root)

	go func() {
		result, err := cleaner.ExecuteWithResult(*plan, opts, func(u cleaner.ProgressUpdate) {
			ws.app.Driver().DoFromGoroutine(func() {
				if u.Total > 0 {
					panel.SetProgress(u.Current, u.Total, "", "groups")
					ws.setTabState(ws.texts.TabCache, &headerState{Task: ws.texts.CacheDeleteTaskProgress(u)})
				}
				if u.Message != "" {
					panel.SetProgress(u.Current, u.Total, u.Message, "groups")
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
			ws.showHistory()
		}, false)
	}()
}
