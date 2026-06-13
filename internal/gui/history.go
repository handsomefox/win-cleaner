package gui

import (
	"fmt"
	"sort"
	"time"

	"fyne.io/fyne/v2"
	"fyne.io/fyne/v2/container"
	"fyne.io/fyne/v2/layout"
	"fyne.io/fyne/v2/theme"
	"fyne.io/fyne/v2/widget"

	"win-clear/internal/cleaner"
)

func (ws *workspace) showHistory() {
	historyHeader := func(task, selection string) *headerState {
		return &headerState{
			Task:       task,
			Selection:  selection,
			ActionText: ws.texts.ActionBack,
			ActionIcon: theme.NavigateBackIcon(),
			Action:     ws.restoreCache,
		}
	}

	results, skipped, err := cleaner.LoadStats()
	if err != nil {
		ws.applyHeaderWidgets(historyHeader(ws.texts.TaskHistoryFailed, ""))
		ws.setContent(centeredStatus(err.Error()))
		return
	}
	sort.Slice(results, func(i, j int) bool {
		return runTimestamp(&results[i]).After(runTimestamp(&results[j]))
	})

	ws.applyHeaderWidgets(historyHeader(ws.texts.TaskHistory, ws.texts.RunsCount(len(results))))

	if len(results) == 0 {
		msg := ws.texts.HistoryNoRuns
		if skipped > 0 {
			msg = fmt.Sprintf("%s %d files could not be read.", ws.texts.HistoryNoRuns, skipped)
		}
		ws.setContent(centeredStatus(msg))
		return
	}

	list := widget.NewList(
		func() int { return len(results) },
		newHistoryListRow,
		func(i int, o fyne.CanvasObject) {
			updateHistoryListRow(ws.texts, o, &results[i])
		},
	)
	listScroll := container.NewVScroll(list)
	listScroll.SetMinSize(fyne.NewSize(360, 500))

	detailsLabel := widget.NewLabel("")
	detailsLabel.Wrapping = fyne.TextWrapWord
	detailsScroll := container.NewVScroll(detailsLabel)
	detailsScroll.SetMinSize(fyne.NewSize(720, 500))

	updateDetails := func(idx int) {
		if idx >= 0 && idx < len(results) {
			detailsLabel.SetText(runDetailsText(ws.texts, &results[idx]))
		}
	}
	list.OnSelected = updateDetails
	list.Select(0)
	updateDetails(0)

	meta := widget.NewLabel(statsSummaryText(ws.texts, results, skipped, time.Now()))
	meta.Wrapping = fyne.TextWrapWord
	refreshBtn := widget.NewButtonWithIcon(ws.texts.ActionRefresh, theme.ViewRefreshIcon(), ws.showHistory)
	metaRow := container.NewBorder(nil, nil, nil, refreshBtn, meta)

	split := container.NewHSplit(listScroll, detailsScroll)
	split.Offset = 0.36
	ws.setContent(container.NewPadded(container.NewBorder(
		metaRow, nil, nil, nil,
		contentPanel(ws.texts.HistoryPreviousRunsTitle, ws.texts.HistoryPreviousRunsSubtitle, split),
	)))
}

func newHistoryListRow() fyne.CanvasObject {
	title := widget.NewLabelWithStyle("", fyne.TextAlignLeading, fyne.TextStyle{Bold: true})
	meta := widget.NewLabel("")
	meta.Importance = widget.LowImportance
	bytes := widget.NewLabelWithStyle("", fyne.TextAlignTrailing, fyne.TextStyle{Bold: true})
	bytes.Importance = widget.HighImportance
	status := widget.NewLabelWithStyle("", fyne.TextAlignTrailing, fyne.TextStyle{})
	return container.NewHBox(
		widget.NewIcon(theme.HistoryIcon()),
		container.NewVBox(title, meta),
		layout.NewSpacer(),
		container.NewVBox(bytes, status),
	)
}

func updateHistoryListRow(texts *uiText, o fyne.CanvasObject, res *cleaner.ExecResult) {
	row, ok := o.(*fyne.Container)
	if !ok || len(row.Objects) < 4 {
		return
	}
	textCol, ok := row.Objects[1].(*fyne.Container)
	if !ok || len(textCol.Objects) < 2 {
		return
	}
	stats, ok := row.Objects[3].(*fyne.Container)
	if !ok || len(stats.Objects) < 2 {
		return
	}
	title, ok := textCol.Objects[0].(*widget.Label)
	if !ok {
		return
	}
	meta, ok := textCol.Objects[1].(*widget.Label)
	if !ok {
		return
	}
	bytes, ok := stats.Objects[0].(*widget.Label)
	if !ok {
		return
	}
	status, ok := stats.Objects[1].(*widget.Label)
	if !ok {
		return
	}

	title.SetText(formatTimestamp(runTimestamp(res)))
	meta.SetText(fmt.Sprintf("%s  |  %s", texts.ItemsCount(res.TotalSelected), formatDuration(res.DurationMs)))
	bytes.SetText(cleaner.HumanBytes(res.TotalBytes))
	if res.ErrorCount > 0 {
		status.Importance = widget.DangerImportance
		status.SetText(texts.ErrorsCount(res.ErrorCount))
	} else {
		status.Importance = widget.LowImportance
		status.SetText(texts.ResultStatusOK)
	}
	status.Refresh()
}
