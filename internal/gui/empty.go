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

func (ws *workspace) showEmptyRootSelect() {
	roots := cleaner.DefaultEmptyFolderRoots()
	if len(roots) == 0 {
		ws.setTabState(ws.texts.TabEmpty, &headerState{Task: ws.texts.EmptyNoRootsTask})
		ws.setTabContent(ws.texts.TabEmpty, centeredStatus(ws.texts.EmptyNoRootsMessage))
		return
	}
	ws.showEmptyRootSelectWithRoots(roots)
}

func (ws *workspace) showEmptyRootSelectWithRoots(roots []cleaner.EmptyFolderRoot) {
	selectedLabel := widget.NewLabel(emptyRootSelectionSummary(ws.texts, roots))

	rows := make([]fyne.CanvasObject, 0, len(roots))
	checks := make([]*widget.Check, len(roots))
	refreshSummary := func() {
		summary := emptyRootSelectionSummary(ws.texts, roots)
		selectedLabel.SetText(summary)
		ws.setTabState(ws.texts.TabEmpty, &headerState{
			Task:       ws.texts.TaskEmptyChooseRoots,
			Selection:  summary,
			ActionText: ws.texts.ActionScan,
			ActionIcon: theme.SearchIcon(),
			Action: func() {
				selected := 0
				for _, root := range roots {
					if root.On {
						selected++
					}
				}
				if selected == 0 {
					dialog.ShowInformation(ws.texts.DialogNoRootsTitle, ws.texts.DialogNoRootsMessage, ws.window)
					return
				}
				ws.showEmptyScan(roots)
			},
		})
	}

	for i := range roots {
		idx := i
		root := &roots[idx]
		chk := widget.NewCheck("", func(checked bool) {
			roots[idx].On = checked
			refreshSummary()
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

	selectAll := widget.NewButtonWithIcon(ws.texts.ActionSelectAll, theme.ContentAddIcon(), func() {
		for i := range roots {
			roots[i].On = true
			checks[i].Checked = true
			checks[i].Refresh()
		}
		refreshSummary()
	})
	selectNone := widget.NewButtonWithIcon(ws.texts.ActionDeselectAll, theme.ContentRemoveIcon(), func() {
		for i := range roots {
			roots[i].On = false
			checks[i].Checked = false
			checks[i].Refresh()
		}
		refreshSummary()
	})

	refreshSummary()
	scroll := container.NewVScroll(container.NewVBox(rows...))
	scroll.SetMinSize(fyne.NewSize(0, 480))
	ws.setTabContent(ws.texts.TabEmpty, container.NewPadded(container.NewBorder(
		container.NewVBox(
			widget.NewLabel(ws.texts.EmptyRootLead),
			container.NewHBox(selectAll, selectNone, layout.NewSpacer(), selectedLabel),
		),
		nil, nil, nil,
		titledCard(ws.texts.EmptyRootsCardTitle, ws.texts.EmptyRootsCardSubtitle, scroll),
	)))
}

func (ws *workspace) showEmptyScan(roots []cleaner.EmptyFolderRoot) {
	status := widget.NewLabel(ws.texts.EmptyScanStatus)
	status.Alignment = fyne.TextAlignCenter
	progress := widget.NewProgressBar()
	var stale atomic.Bool

	ws.setTabState(ws.texts.TabEmpty, &headerState{
		Task:       ws.texts.TaskEmptyScanning,
		Selection:  emptyRootSelectionSummary(ws.texts, roots),
		ActionText: ws.texts.ActionCancel,
		ActionIcon: theme.CancelIcon(),
		Action: func() {
			stale.Store(true)
			ws.showEmptyRootSelectWithRoots(roots)
		},
	})
	ws.setTabContent(ws.texts.TabEmpty, container.NewPadded(titledCard(ws.texts.EmptyScanCardTitle, ws.texts.EmptyScanCardSubtitle, container.NewPadded(container.NewVBox(status, progress)))))

	go func() {
		plan := cleaner.BuildEmptyFolderPlanWithCancel(roots, stale.Load, func(u cleaner.ProgressUpdate) {
			if stale.Load() {
				return
			}
			ws.app.Driver().DoFromGoroutine(func() {
				if stale.Load() {
					return
				}
				if u.Total > 0 {
					progress.SetValue(float64(u.Current) / float64(u.Total))
					ws.setTabState(ws.texts.TabEmpty, &headerState{Task: ws.texts.EmptyScanTaskProgress(u)})
				}
				if u.Message != "" {
					status.SetText(ws.texts.EmptyScanProgress(u))
				}
			}, false)
		})
		if stale.Load() {
			return
		}
		ws.app.Driver().DoFromGoroutine(func() {
			if stale.Load() {
				return
			}
			ws.showEmptySelect(&plan)
		}, false)
	}()
}

func (ws *workspace) showEmptySelect(plan *cleaner.EmptyFolderPlan) {
	filterEntry := widget.NewEntry()
	filterEntry.SetPlaceHolder(ws.texts.EmptySearchPlaceholder)
	var listScroll *container.Scroll
	var remove func()

	updateHeader := func(remove func()) {
		selection, savings := emptySelectionSummary(ws.texts, plan)
		ws.setTabState(ws.texts.TabEmpty, &headerState{
			Task:       ws.texts.TaskEmptyReview,
			Selection:  selection,
			Savings:    savings,
			ActionText: ws.texts.ActionRemove,
			ActionIcon: theme.DeleteIcon(),
			Action:     remove,
		})
	}

	rebuildList := func(filter string) {
		filter = normalizedFilter(filter)
		rows := make([]fyne.CanvasObject, 0, len(plan.Folders))
		for i := range plan.Folders {
			if filter != "" && !containsNormalized(plan.Folders[i].Path, filter) {
				continue
			}
			idx := i
			folder := &plan.Folders[idx]
			chk := widget.NewCheck("", func(checked bool) {
				plan.Folders[idx].On = checked
				updateHeader(remove)
			})
			chk.Checked = folder.On
			pathLabel := widget.NewLabel(folder.Path)
			pathLabel.Truncation = fyne.TextTruncateEllipsis
			rows = append(rows, container.NewBorder(nil, nil, chk, nil, pathLabel))
		}
		if len(rows) == 0 {
			rows = append(rows, centeredStatus(ws.texts.NoEmptyFoldersFound))
		}
		content := container.NewVBox(rows...)
		if listScroll == nil {
			listScroll = container.NewVScroll(content)
		} else {
			listScroll.Content = content
			listScroll.Refresh()
		}
	}

	remove = func() {
		plan.Selected = countSelectedEmptyFolders(plan)
		if plan.Selected == 0 {
			dialog.ShowInformation(ws.texts.DialogNothingSelectedTitle, ws.texts.DialogSelectEmptyFolder, ws.window)
			return
		}
		dialog.NewConfirm(ws.texts.DialogConfirmEmptyTitle, ws.texts.ConfirmEmptyRemoval(plan.Selected), func(ok bool) {
			if ok {
				ws.showEmptyDelete(plan)
			}
		}, ws.window).Show()
	}

	filterEntry.OnChanged = func(s string) {
		rebuildList(s)
		updateHeader(remove)
	}
	selectAll := widget.NewButtonWithIcon(ws.texts.ActionSelectAll, theme.ContentAddIcon(), func() {
		for i := range plan.Folders {
			plan.Folders[i].On = true
		}
		rebuildList(filterEntry.Text)
		updateHeader(remove)
	})
	selectNone := widget.NewButtonWithIcon(ws.texts.ActionDeselectAll, theme.ContentRemoveIcon(), func() {
		for i := range plan.Folders {
			plan.Folders[i].On = false
		}
		rebuildList(filterEntry.Text)
		updateHeader(remove)
	})
	rootsBtn := widget.NewButtonWithIcon(ws.texts.ActionRoots, theme.NavigateBackIcon(), func() {
		ws.showEmptyRootSelectWithRoots(plan.Roots)
	})
	errLabel := widget.NewLabel(emptyScanStatusText(ws.texts, plan))
	errLabel.Wrapping = fyne.TextWrapWord

	rebuildList("")
	updateHeader(remove)
	listScroll.SetMinSize(fyne.NewSize(0, 480))
	searchRow := labeledEntryRow(ws.texts.LabelSearch, filterEntry)
	actions := toolbarRow(selectAll, selectNone, layout.NewSpacer(), rootsBtn)
	ws.setTabContent(ws.texts.TabEmpty, container.NewPadded(container.NewBorder(
		container.NewVBox(searchRow, actions, errLabel),
		nil, nil, nil,
		titledCard(ws.texts.EmptyFoldersCardTitle, ws.texts.EmptyFoldersCardSubtitle, listScroll),
	)))
}

func (ws *workspace) showEmptyDelete(plan *cleaner.EmptyFolderPlan) {
	plan.Selected = countSelectedEmptyFolders(plan)
	panel := newCleanupProgressPanel(
		ws.texts.EmptyDeleteCardTitle,
		ws.texts.EmptyDeleteCardSubtitle,
		fmt.Sprintf("%d folders", plan.Selected),
		"Empty folders",
	)
	panel.SetProgress(0, plan.Selected, ws.texts.StatusPreparing, "folders")

	ws.setTabState(ws.texts.TabEmpty, &headerState{Task: ws.texts.TaskEmptyDeleting})
	ws.setTabContent(ws.texts.TabEmpty, panel.root)

	go func() {
		result := cleaner.ExecuteEmptyFolderPlan(plan, func(u cleaner.ProgressUpdate) {
			ws.app.Driver().DoFromGoroutine(func() {
				if u.Total > 0 {
					panel.SetProgress(u.Current, u.Total, "", "folders")
					ws.setTabState(ws.texts.TabEmpty, &headerState{Task: ws.texts.EmptyDeleteTaskProgress(u)})
				}
				if u.Message != "" {
					panel.SetProgress(u.Current, u.Total, u.Message, "folders")
				}
			}, false)
		})
		ws.app.Driver().DoFromGoroutine(func() {
			ws.showEmptyResults(&result)
		}, false)
	}()
}

func (ws *workspace) showEmptyResults(result *cleaner.EmptyFolderResult) {
	headline, summary := emptyResultSummary(ws.texts, result)
	headlineLabel := widget.NewLabelWithStyle(headline, fyne.TextAlignLeading, fyne.TextStyle{Bold: true})
	summaryLabel := widget.NewLabel(summary)

	errorBox := container.NewVBox()
	if len(result.Errors) > 0 {
		var detail strings.Builder
		for _, err := range result.Errors {
			fmt.Fprintf(&detail, "- %s: %s\n", err.Path, err.Error)
		}
		errorBox.Add(widget.NewButtonWithIcon(ws.texts.ResultErrorDetails, theme.WarningIcon(), func() {
			label := widget.NewLabel(detail.String())
			label.Wrapping = fyne.TextWrapWord
			scroll := container.NewVScroll(label)
			scroll.SetMinSize(fyne.NewSize(720, 360))
			dialog.NewCustom(ws.texts.DialogEmptyErrors, ws.texts.DialogClose, scroll, ws.window).Show()
		}))
	}

	ws.setTabState(ws.texts.TabEmpty, &headerState{
		Task:       headline,
		Selection:  removedSummary(result.Removed),
		ActionText: ws.texts.ActionScanAgain,
		ActionIcon: theme.ViewRefreshIcon(),
		Action:     ws.showEmptyRootSelect,
	})
	ws.setTabContent(ws.texts.TabEmpty, container.NewPadded(container.NewBorder(
		nil,
		container.NewHBox(
			widget.NewButtonWithIcon(ws.texts.ActionCacheCleanup, theme.NavigateBackIcon(), func() { ws.selectTab(ws.texts.TabCache) }),
			layout.NewSpacer(),
			widget.NewButtonWithIcon(ws.texts.ActionClose, theme.ConfirmIcon(), func() { ws.safeClose(nil) }),
		),
		nil, nil,
		titledCard(headline, summary, container.NewPadded(container.NewVBox(headlineLabel, summaryLabel, errorBox))),
	)))
}
