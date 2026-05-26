package gui

import (
	"fyne.io/fyne/v2"
	"fyne.io/fyne/v2/container"
	"fyne.io/fyne/v2/theme"
	"fyne.io/fyne/v2/widget"

	"win-clear/internal/cleaner"
)

type headerState struct {
	Task       string
	Selection  string
	Savings    string
	ActionText string
	ActionIcon fyne.Resource
	Action     func()
}

type workspace struct {
	app       fyne.App
	window    fyne.Window
	reg       cleaner.Registry
	opts      cleaner.Options
	texts     *uiText
	safeClose func(error)

	tabs       *container.AppTabs
	cacheTab   *container.TabItem
	emptyTab   *container.TabItem
	historyTab *container.TabItem

	taskLabel      *widget.Label
	selectionLabel *widget.Label
	savingsLabel   *widget.Label
	actionButton   *widget.Button
	states         map[string]headerState
}

func newWorkspace(a fyne.App, w fyne.Window, reg cleaner.Registry, opts cleaner.Options, texts *uiText, safeClose func(error)) *workspace {
	ws := &workspace{
		app:            a,
		window:         w,
		reg:            reg,
		opts:           opts,
		texts:          texts,
		safeClose:      safeClose,
		taskLabel:      widget.NewLabel(texts.TaskReady),
		selectionLabel: widget.NewLabel(""),
		savingsLabel:   widget.NewLabelWithStyle("", fyne.TextAlignTrailing, fyne.TextStyle{Bold: true}),
		actionButton:   widget.NewButtonWithIcon(texts.ActionStart, theme.MediaPlayIcon(), nil),
		states:         map[string]headerState{},
	}
	ws.actionButton.Importance = widget.HighImportance
	ws.actionButton.Disable()
	ws.cacheTab = container.NewTabItemWithIcon(texts.TabCache, theme.StorageIcon(), centeredStatus(texts.PreparingCache))
	ws.emptyTab = container.NewTabItemWithIcon(texts.TabEmpty, theme.FolderIcon(), centeredStatus(texts.PreparingEmpty))
	ws.historyTab = container.NewTabItemWithIcon(texts.TabHistory, theme.HistoryIcon(), centeredStatus(texts.LoadingHistory))
	ws.tabs = container.NewAppTabs(ws.cacheTab, ws.emptyTab, ws.historyTab)
	ws.tabs.SetTabLocation(container.TabLocationTop)
	ws.tabs.OnSelected = func(item *container.TabItem) {
		ws.applyHeader(item.Text)
		if item.Text == texts.TabHistory {
			ws.showHistory()
		}
	}

	w.SetContent(container.NewBorder(ws.header(), nil, nil, nil, ws.tabs))
	ws.setTabState(texts.TabCache, &headerState{Task: texts.TaskCacheScanning})
	ws.setTabState(texts.TabEmpty, &headerState{Task: texts.TaskEmptyChooseRoots})
	ws.setTabState(texts.TabHistory, &headerState{Task: texts.TaskHistory})
	ws.applyHeader(texts.TabCache)
	return ws
}

func (ws *workspace) header() fyne.CanvasObject {
	title := widget.NewLabelWithStyle(ws.texts.AppTitle, fyne.TextAlignLeading, fyne.TextStyle{Bold: true})
	ws.selectionLabel.Hide()
	ws.savingsLabel.Hide()
	return headerBar(title, ws.taskLabel, ws.selectionLabel, ws.savingsLabel, ws.actionButton)
}

func (ws *workspace) setTabState(tab string, state *headerState) {
	ws.states[tab] = *state
	if ws.activeTab() == tab {
		ws.applyHeader(tab)
	}
}

func (ws *workspace) applyHeader(tab string) {
	state := ws.states[tab]
	ws.taskLabel.SetText(state.Task)
	ws.selectionLabel.SetText(state.Selection)
	ws.savingsLabel.SetText(state.Savings)
	setLabelVisible(ws.selectionLabel, state.Selection != "")
	setLabelVisible(ws.savingsLabel, state.Savings != "")
	if state.ActionText == "" {
		ws.actionButton.SetText(ws.texts.ActionNone)
		ws.actionButton.SetIcon(theme.CancelIcon())
		ws.actionButton.OnTapped = nil
		ws.actionButton.Hide()
		return
	}
	ws.actionButton.Show()
	ws.actionButton.SetText(state.ActionText)
	if state.ActionIcon != nil {
		ws.actionButton.SetIcon(state.ActionIcon)
	}
	ws.actionButton.OnTapped = state.Action
	if state.Action == nil {
		ws.actionButton.Disable()
	} else {
		ws.actionButton.Enable()
	}
}

func setLabelVisible(label *widget.Label, visible bool) {
	if visible {
		label.Show()
		return
	}
	label.Hide()
}

func (ws *workspace) activeTab() string {
	if ws.tabs == nil || ws.tabs.Selected() == nil {
		return ""
	}
	return ws.tabs.Selected().Text
}

func (ws *workspace) setTabContent(tab string, content fyne.CanvasObject) {
	switch tab {
	case ws.texts.TabCache:
		ws.cacheTab.Content = content
	case ws.texts.TabEmpty:
		ws.emptyTab.Content = content
	case ws.texts.TabHistory:
		ws.historyTab.Content = content
	}
	ws.tabs.Refresh()
}

func (ws *workspace) selectTab(tab string) {
	switch tab {
	case ws.texts.TabCache:
		ws.tabs.Select(ws.cacheTab)
	case ws.texts.TabEmpty:
		ws.tabs.Select(ws.emptyTab)
	case ws.texts.TabHistory:
		ws.tabs.Select(ws.historyTab)
	}
}
