package gui

import (
	"fmt"
	"strings"

	"fyne.io/fyne/v2"
	"fyne.io/fyne/v2/container"
	"fyne.io/fyne/v2/dialog"
	"fyne.io/fyne/v2/layout"
	"fyne.io/fyne/v2/theme"
	"fyne.io/fyne/v2/widget"

	"win-clear/internal/cleaner"
)

func (ws *workspace) showResults(result *cleaner.ExecResult, execErr error) {
	headline, summary := cleanupResultSummary(ws.texts, result, execErr)
	headlineLabel := widget.NewLabelWithStyle(headline, fyne.TextAlignLeading, fyne.TextStyle{Bold: true})
	summaryLabel := widget.NewLabel(summary)

	detailRows := make([]fyne.CanvasObject, 0, len(result.Groups))
	for i := range result.Groups {
		group := result.Groups[i]
		detailRows = append(detailRows, resultGroupRow(ws.texts, &group, ws.window))
	}
	if len(detailRows) == 0 {
		detailRows = append(detailRows, centeredStatus(ws.texts.ResultNoGroupDetails))
	}
	detailScroll := container.NewVScroll(container.NewVBox(detailRows...))
	detailScroll.SetMinSize(fyne.NewSize(0, 380))

	errorSummary, errorDetails := buildErrorSummary(result)
	errorBox := container.NewVBox()
	if errorSummary != "" {
		errLabel := widget.NewLabel(errorSummary)
		errLabel.Wrapping = fyne.TextWrapWord
		errorBox.Add(widget.NewSeparator())
		errorBox.Add(errLabel)
		errorBox.Add(widget.NewButtonWithIcon(ws.texts.ResultErrorDetails, theme.WarningIcon(), func() {
			label := widget.NewLabel(errorDetails)
			label.Wrapping = fyne.TextWrapWord
			scroll := container.NewVScroll(label)
			scroll.SetMinSize(fyne.NewSize(720, 360))
			dialog.NewCustom(ws.texts.DialogCleanupErrors, ws.texts.DialogClose, scroll, ws.window).Show()
		}))
	}

	ws.setTabState(ws.texts.TabCache, &headerState{
		Task:       headline,
		Selection:  fmt.Sprintf("%d groups", result.TotalSelected),
		Savings:    "Freed: " + cleaner.HumanBytes(result.TotalBytes),
		ActionText: ws.texts.ActionCleanAgain,
		ActionIcon: theme.ViewRefreshIcon(),
		Action:     ws.showCacheScan,
	})
	ws.setTabContent(ws.texts.TabCache, container.NewPadded(container.NewBorder(
		nil,
		container.NewHBox(
			widget.NewButtonWithIcon(ws.texts.ActionHistory, theme.HistoryIcon(), func() { ws.selectTab(ws.texts.TabHistory) }),
			layout.NewSpacer(),
			widget.NewButtonWithIcon(ws.texts.ActionClose, theme.ConfirmIcon(), func() { ws.safeClose(execErr) }),
		),
		nil, nil,
		widget.NewCard(headline, summary, container.NewPadded(container.NewVBox(headlineLabel, summaryLabel, errorBox, widget.NewSeparator(), detailScroll))),
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
