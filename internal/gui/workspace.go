package gui

import (
	"fyne.io/fyne/v2"
	"fyne.io/fyne/v2/canvas"
	"fyne.io/fyne/v2/container"
	"fyne.io/fyne/v2/theme"
	"fyne.io/fyne/v2/widget"

	"win-clear/internal/cleaner"
)

type headerState struct {
	Task         string
	Selection    string
	Savings      string
	SavingsBytes uint64
	ActionText   string
	ActionIcon   fyne.Resource
	Action       func()
}

type workspace struct {
	app       fyne.App
	window    fyne.Window
	reg       cleaner.Registry
	opts      cleaner.Options
	texts     *uiText
	safeClose func(error)

	contentHolder *fyne.Container

	taskLabel      *widget.Label
	selectionLabel *widget.Label
	savingsText    *canvas.Text
	selectionChip  fyne.CanvasObject
	savingsChip    fyne.CanvasObject
	actionButton   *widget.Button
	actionWrap     fyne.CanvasObject // fixed-width holder around actionButton

	// The current Cache Cleanup screen, retained so returning from History
	// (via the burger menu) restores exactly where the user was.
	cacheContent fyne.CanvasObject
	cacheState   headerState
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
		savingsText:    newSavingsText(),
		actionButton:   widget.NewButtonWithIcon(texts.ActionStart, theme.MediaPlayIcon(), nil),
	}
	ws.actionButton.Importance = widget.HighImportance
	ws.actionButton.Disable()
	ws.contentHolder = container.NewStack(centeredStatus(texts.PreparingCache))

	w.SetContent(container.NewBorder(ws.header(), nil, nil, nil, ws.contentHolder))
	ws.applyHeaderWidgets(&headerState{Task: texts.TaskCacheScanning})
	return ws
}

func (ws *workspace) header() fyne.CanvasObject {
	burger := widget.NewButtonWithIcon("", theme.MenuIcon(), nil)
	burger.Importance = widget.LowImportance
	burger.OnTapped = func() { ws.showBurgerMenu(burger) }

	title := widget.NewLabelWithStyle(ws.texts.AppTitle, fyne.TextAlignLeading, fyne.TextStyle{Bold: true})
	title.SizeName = theme.SizeNameHeadingText
	titleRow := container.NewHBox(burger, title)

	ws.taskLabel.Importance = widget.LowImportance
	ws.selectionChip = chip(ws.selectionLabel)
	ws.savingsChip = chip(ws.savingsText)
	ws.selectionChip.Hide()
	ws.savingsChip.Hide()

	// Fixed width so toggling the action label (Preview ⇄ Clean Up …) never
	// shifts the header layout.
	ws.actionWrap = container.NewGridWrap(
		fyne.NewSize(actionButtonWidth(ws.texts), ws.actionButton.MinSize().Height),
		ws.actionButton,
	)
	return headerBar(titleRow, ws.taskLabel, ws.selectionChip, ws.savingsChip, ws.actionWrap)
}

func (ws *workspace) showBurgerMenu(anchor fyne.CanvasObject) {
	menu := fyne.NewMenu(
		"",
		fyne.NewMenuItemWithIcon(ws.texts.MenuCacheCleanup, theme.StorageIcon(), ws.restoreCache),
		fyne.NewMenuItemWithIcon(ws.texts.MenuHistory, theme.HistoryIcon(), ws.showHistory),
		fyne.NewMenuItemSeparator(),
		fyne.NewMenuItemWithIcon(ws.texts.MenuAbout, theme.InfoIcon(), func() { showAbout(ws.window, ws.texts) }),
		fyne.NewMenuItemWithIcon(ws.texts.MenuQuit, theme.CancelIcon(), func() { ws.safeClose(cleaner.ErrCancelled) }),
	)
	pos := fyne.CurrentApp().Driver().AbsolutePositionForObject(anchor)
	pos.Y += anchor.Size().Height
	widget.ShowPopUpMenuAtPosition(menu, ws.window.Canvas(), pos)
}

func (ws *workspace) applyHeaderWidgets(state *headerState) {
	ws.taskLabel.SetText(state.Task)
	setObjectVisible(ws.taskLabel, state.Task != "")
	ws.selectionLabel.SetText(state.Selection)
	ws.savingsText.Text = state.Savings
	ws.savingsText.Color = magnitudeColor(state.SavingsBytes)
	ws.savingsText.Refresh()
	setObjectVisible(ws.selectionChip, state.Selection != "")
	setObjectVisible(ws.savingsChip, state.Savings != "")
	if state.ActionText == "" {
		ws.actionButton.OnTapped = nil
		ws.actionButton.Hide()
		setObjectVisible(ws.actionWrap, false)
		return
	}
	setObjectVisible(ws.actionWrap, true)
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

func (ws *workspace) setContent(content fyne.CanvasObject) {
	ws.contentHolder.Objects = []fyne.CanvasObject{content}
	ws.contentHolder.Refresh()
}

// showCache displays a Cache Cleanup screen and records it for later restore.
func (ws *workspace) showCache(content fyne.CanvasObject, state *headerState) {
	ws.cacheContent = content
	ws.cacheState = *state
	ws.setContent(content)
	ws.applyHeaderWidgets(state)
}

// updateCacheHeader refreshes the header for the current cache screen (e.g. live
// progress or selection changes) without rebuilding its content.
func (ws *workspace) updateCacheHeader(state *headerState) {
	ws.cacheState = *state
	ws.applyHeaderWidgets(state)
}

// setCacheContent swaps the cache screen body while keeping the current header.
func (ws *workspace) setCacheContent(content fyne.CanvasObject) {
	ws.cacheContent = content
	ws.setContent(content)
}

// restoreCache returns to the saved Cache Cleanup screen (used by the burger menu
// and the History "Back" action).
func (ws *workspace) restoreCache() {
	if ws.cacheContent == nil {
		return
	}
	ws.setContent(ws.cacheContent)
	ws.applyHeaderWidgets(&ws.cacheState)
}

func setObjectVisible(obj fyne.CanvasObject, visible bool) {
	if obj == nil {
		return
	}
	if visible {
		obj.Show()
		return
	}
	obj.Hide()
}
