package gui

import (
	"fmt"
	"image/color"
	"math"
	"strings"

	"fyne.io/fyne/v2"
	"fyne.io/fyne/v2/canvas"
	"fyne.io/fyne/v2/container"
	"fyne.io/fyne/v2/dialog"
	"fyne.io/fyne/v2/layout"
	"fyne.io/fyne/v2/theme"
	"fyne.io/fyne/v2/widget"

	"win-clear/internal/cleaner"
)

// surfaceCard wraps content in a rounded, bordered surface panel — the building
// block that replaces flat widget.Card / separator stacks across the UI.
func surfaceCard(content fyne.CanvasObject) fyne.CanvasObject {
	bg := canvas.NewRectangle(colSurface)
	bg.CornerRadius = 14
	bg.StrokeColor = colBorder
	bg.StrokeWidth = 1
	return container.NewStack(bg, container.NewPadded(content))
}

// chip renders a compact rounded pill around content, used for header stats.
func chip(content fyne.CanvasObject) fyne.CanvasObject {
	bg := canvas.NewRectangle(colSurfaceRaised)
	bg.CornerRadius = 9
	bg.StrokeColor = colBorder
	bg.StrokeWidth = 1
	return container.NewStack(bg, container.NewPadded(content))
}

// titledCard is a surfaceCard with a bold title, muted subtitle, and body.
func titledCard(title, subtitle string, body fyne.CanvasObject) fyne.CanvasObject {
	parts := []fyne.CanvasObject{widget.NewLabelWithStyle(title, fyne.TextAlignLeading, fyne.TextStyle{Bold: true})}
	if subtitle != "" {
		sub := widget.NewLabel(subtitle)
		sub.Wrapping = fyne.TextWrapWord
		sub.Importance = widget.LowImportance
		parts = append(parts, sub)
	}
	head := container.NewVBox(append(parts, widget.NewSeparator())...)
	return surfaceCard(container.NewBorder(head, nil, nil, nil, body))
}

func headerBar(title fyne.CanvasObject, task *widget.Label, selectionChip, savingsChip, action fyne.CanvasObject) fyne.CanvasObject {
	bg := canvas.NewRectangle(colSurface)
	topRow := container.NewBorder(nil, nil, title, container.NewHBox(selectionChip, savingsChip, action))
	inner := container.NewPadded(container.NewVBox(topRow, task))
	return container.NewVBox(container.NewStack(bg, inner), widget.NewSeparator())
}

func centeredStatus(text string) fyne.CanvasObject {
	label := widget.NewLabel(text)
	label.Alignment = fyne.TextAlignCenter
	return container.NewCenter(label)
}

func toolbarRow(objects ...fyne.CanvasObject) fyne.CanvasObject {
	return container.NewHBox(objects...)
}

func contentPanel(title, subtitle string, body fyne.CanvasObject) fyne.CanvasObject {
	return titledCard(title, subtitle, body)
}

func scanPanel(title, subtitle string, body ...fyne.CanvasObject) fyne.CanvasObject {
	return container.NewPadded(contentPanel(title, subtitle, container.NewPadded(container.NewVBox(body...))))
}

type cleanupProgressPanel struct {
	root     fyne.CanvasObject
	progress *widget.ProgressBar
	percent  *widget.Label
	count    *widget.Label
	current  *widget.Label
}

func newCleanupProgressPanel(texts *uiText, title, subtitle, selectedText, scopeText string) *cleanupProgressPanel {
	progress := widget.NewProgressBar()
	percent := widget.NewLabelWithStyle("0%", fyne.TextAlignTrailing, fyne.TextStyle{Bold: true})
	percent.SizeName = theme.SizeNameHeadingText
	percent.Importance = widget.HighImportance
	count := widget.NewLabelWithStyle("0 / 0", fyne.TextAlignLeading, fyne.TextStyle{Bold: true})
	current := widget.NewLabel(texts.StatusPreparing)
	current.Wrapping = fyne.TextWrapWord
	destination := widget.NewLabel(texts.RecycleBin)

	status := container.NewBorder(
		nil, nil,
		container.NewHBox(widget.NewIcon(theme.DeleteIcon()), container.NewVBox(
			widget.NewLabelWithStyle(texts.ProgressMovingTitle, fyne.TextAlignLeading, fyne.TextStyle{Bold: true}),
			current,
		)),
		percent,
		nil,
	)
	stats := container.NewGridWithColumns(
		3,
		progressStat(theme.CheckButtonCheckedIcon(), texts.StatSelected, selectedText),
		progressStat(theme.StorageIcon(), texts.StatScope, scopeText),
		progressStat(theme.DeleteIcon(), texts.StatDestination, texts.RecycleBin),
	)
	body := container.NewPadded(container.NewVBox(
		status,
		progress,
		container.NewBorder(nil, nil, count, destination, nil),
		widget.NewSeparator(),
		stats,
	))

	return &cleanupProgressPanel{
		root:     container.NewPadded(contentPanel(title, subtitle, body)),
		progress: progress,
		percent:  percent,
		count:    count,
		current:  current,
	}
}

func progressStat(icon fyne.Resource, label, value string) fyne.CanvasObject {
	valueLabel := widget.NewLabelWithStyle(value, fyne.TextAlignLeading, fyne.TextStyle{Bold: true})
	labelWidget := widget.NewLabel(label)
	labelWidget.Importance = widget.LowImportance
	return chip(container.NewHBox(widget.NewIcon(icon), container.NewVBox(valueLabel, labelWidget)))
}

func (p *cleanupProgressPanel) SetProgress(current, total int, message, unit string) {
	if total > 0 {
		value := float64(current) / float64(total)
		p.progress.SetValue(value)
		p.percent.SetText(fmt.Sprintf("%d%%", int(math.Round(value*100))))
		p.count.SetText(fmt.Sprintf("%d / %d %s", current, total, unit))
	} else {
		p.progress.SetValue(0)
		p.percent.SetText("0%")
		p.count.SetText("0 / 0 " + unit)
	}
	if message != "" {
		p.current.SetText(message)
	}
}

const rowIndentStep = 32 // per-level left indent for the cache tree

func hspace(w float32) fyne.CanvasObject {
	return container.NewGridWrap(fyne.NewSize(w, 1), canvas.NewRectangle(colTransparent))
}

func clamp01(t float64) float64 {
	if t < 0 {
		return 0
	}
	if t > 1 {
		return 1
	}
	return t
}

// magnitudeColor maps a byte size to a white→red tone on a log scale, so larger
// targets read "hotter" and are easy to spot.
func magnitudeColor(bytes uint64) color.Color {
	if bytes == 0 {
		return colMuted
	}
	const lo, hi = 20.0, 33.0 // ~1 MiB (white) → ~8 GiB (red)
	t := clamp01((math.Log2(float64(bytes)) - lo) / (hi - lo))
	lerp := func(a, b uint8) uint8 { return uint8(float64(a) + (float64(b)-float64(a))*t) }
	return color.NRGBA{R: lerp(colText.R, 0xff), G: lerp(colText.G, 0x5c), B: lerp(colText.B, 0x5c), A: 0xff}
}

// sizeCell renders a right-aligned size value tinted by its magnitude.
func sizeCell(texts *uiText, bytes uint64) fyne.CanvasObject {
	if bytes == 0 {
		t := canvas.NewText(texts.NotFound, colMuted)
		t.TextStyle = fyne.TextStyle{Italic: true}
		t.Alignment = fyne.TextAlignTrailing
		return t
	}
	t := canvas.NewText(cleaner.HumanBytes(bytes), magnitudeColor(bytes))
	t.TextStyle = fyne.TextStyle{Bold: true}
	t.Alignment = fyne.TextAlignTrailing
	return t
}

// newSavingsText is the header's "Est. savings" value, tinted by magnitude.
func newSavingsText() *canvas.Text {
	t := canvas.NewText("", colText)
	t.TextStyle = fyne.TextStyle{Bold: true}
	t.Alignment = fyne.TextAlignTrailing
	return t
}

// actionButtonWidth returns a width wide enough for the longest action label, so
// the header button keeps a fixed size as its text changes between screens.
func actionButtonWidth(texts *uiText) float32 {
	labels := []string{
		texts.ActionStart, texts.ActionCancel, texts.ActionCleanUp,
		texts.ActionPreview, texts.ActionCleanAgain, texts.ActionDone,
		texts.ActionBack, texts.ActionRefresh, texts.ActionNone,
	}
	var widest float32
	for _, l := range labels {
		if w := fyne.MeasureText(l, theme.TextSize(), fyne.TextStyle{}).Width; w > widest {
			widest = w
		}
	}
	return widest + 72 // icon + button paddings + margin
}

// cacheRow renders one tree row: an optional zebra stripe behind an indented
// leading cluster and a right-aligned trailing (size) cell.
func cacheRow(level int, striped bool, leading, trailing fyne.CanvasObject) fyne.CanvasObject {
	indented := container.NewHBox(hspace(float32(level)*rowIndentStep), leading)
	row := container.NewBorder(nil, nil, indented, trailing)
	if striped {
		return container.NewStack(canvas.NewRectangle(colRowAlt), container.NewPadded(row))
	}
	return container.NewPadded(row)
}

// tappableRow makes its content toggle on tap (used for expandable headers).
type tappableRow struct {
	widget.BaseWidget
	content fyne.CanvasObject
	onTap   func()
}

func newTappableRow(content fyne.CanvasObject, onTap func()) *tappableRow {
	r := &tappableRow{content: content, onTap: onTap}
	r.ExtendBaseWidget(r)
	return r
}

func (r *tappableRow) CreateRenderer() fyne.WidgetRenderer {
	return widget.NewSimpleRenderer(r.content)
}

func (r *tappableRow) Tapped(*fyne.PointEvent) {
	if r.onTap != nil {
		r.onTap()
	}
}

func cacheCategorySection(texts *uiText, category cacheCategoryView, striped bool, expanded map[string]bool, appRows []fyne.CanvasObject, toggleSelected func()) fyne.CanvasObject {
	appsBox := container.NewVBox(appRows...)
	if !expanded[category.Name] {
		appsBox.Hide()
	}

	chevron := widget.NewIcon(expandIcon(expanded[category.Name]))
	selectBtn := widget.NewButtonWithIcon("", cacheCategorySelectionIcon(category.Groups), toggleSelected)
	selectBtn.Importance = widget.LowImportance

	name := widget.NewLabelWithStyle(category.Name, fyne.TextAlignLeading, fyne.TextStyle{Bold: true})
	count := widget.NewLabel(texts.AppsCount(len(category.Groups)))
	count.Importance = widget.LowImportance

	leading := container.NewHBox(selectBtn, chevron, widget.NewIcon(categoryIcon(texts, category.Name)), name, count)
	header := newTappableRow(cacheRow(0, striped, leading, sizeCell(texts, category.Bytes)), func() {
		expanded[category.Name] = !expanded[category.Name]
		if expanded[category.Name] {
			appsBox.Show()
		} else {
			appsBox.Hide()
		}
		chevron.SetResource(expandIcon(expanded[category.Name]))
	})
	return container.NewVBox(header, appsBox)
}

func cacheAppSection(texts *uiText, appName string, groups []*cleaner.Group, striped bool, expanded map[string]bool, w fyne.Window, changed func()) fyne.CanvasObject {
	appSelect := widget.NewButtonWithIcon("", cacheAppSelectionIcon(groups), nil)
	appSelect.Importance = widget.LowImportance
	itemChecks := make([]*widget.Check, len(groups))
	itemRows := make([]fyne.CanvasObject, 0, len(groups))
	onCount := 0
	var total uint64
	for _, grp := range groups {
		total += grp.Bytes
	}

	for i, group := range groups {
		grp := group
		if grp.On {
			onCount++
		}

		chk := widget.NewCheck("", nil)
		chk.Checked = grp.On
		itemChecks[i] = chk
		chk.OnChanged = func(checked bool) {
			grp.On = checked
			appSelect.SetIcon(cacheAppSelectionIcon(groups))
			changed()
		}

		details := texts.ResultDetails
		if len(grp.Errs) > 0 {
			details = texts.DetailsWithIssues(len(grp.Errs))
		}
		detailButton := widget.NewButtonWithIcon(details, theme.InfoIcon(), func() {
			dialog.ShowInformation(grp.App+" - "+grp.Label, cacheGroupDetails(texts, grp), w)
		})
		detailButton.Importance = widget.LowImportance

		leading := container.NewHBox(chk, widget.NewLabel(grp.Label))
		trailing := container.NewHBox(detailButton, sizeCell(texts, grp.Bytes))
		itemRows = append(itemRows, cacheRow(2, i%2 == 1, leading, trailing))
	}

	appSelect.OnTapped = func() {
		checked := !allCacheGroupsSelected(groups)
		for i, group := range groups {
			group.On = checked
			itemChecks[i].Checked = checked
			itemChecks[i].Refresh()
		}
		appSelect.SetIcon(cacheAppSelectionIcon(groups))
		changed()
	}

	itemsBox := container.NewVBox(itemRows...)
	if !expanded[appName] {
		itemsBox.Hide()
	}
	chevron := widget.NewIcon(expandIcon(expanded[appName]))

	name := widget.NewLabelWithStyle(appName, fyne.TextAlignLeading, fyne.TextStyle{Bold: true})
	selected := widget.NewLabel(texts.SelectedOfCount(onCount, len(groups)))
	selected.Importance = widget.LowImportance

	leading := container.NewHBox(appSelect, chevron, name, selected)
	header := newTappableRow(cacheRow(1, striped, leading, sizeCell(texts, total)), func() {
		expanded[appName] = !expanded[appName]
		if expanded[appName] {
			itemsBox.Show()
		} else {
			itemsBox.Hide()
		}
		chevron.SetResource(expandIcon(expanded[appName]))
	})
	return container.NewVBox(header, itemsBox)
}

func allCacheGroupsSelected(groups []*cleaner.Group) bool {
	if len(groups) == 0 {
		return false
	}
	for _, group := range groups {
		if !group.On {
			return false
		}
	}
	return true
}

func cacheAppSelectionIcon(groups []*cleaner.Group) fyne.Resource {
	selected := 0
	for _, group := range groups {
		if group.On {
			selected++
		}
	}
	switch {
	case selected == 0:
		return theme.CheckButtonIcon()
	case selected == len(groups):
		return theme.CheckButtonCheckedIcon()
	default:
		return theme.Current().Icon(theme.IconNameCheckButtonPartial)
	}
}

func cacheCategorySelectionIcon(groups []cleaner.AppGroup) fyne.Resource {
	selected, total := categorySelectionCounts(groups)
	switch {
	case total == 0 || selected == 0:
		return theme.CheckButtonIcon()
	case selected == total:
		return theme.CheckButtonCheckedIcon()
	default:
		return theme.Current().Icon(theme.IconNameCheckButtonPartial)
	}
}

func categorySelectionCounts(groups []cleaner.AppGroup) (selected, total int) {
	for _, appGroup := range groups {
		for _, group := range appGroup.Items {
			total++
			if group.On {
				selected++
			}
		}
	}
	return selected, total
}

func allCategoryGroupsSelected(groups []cleaner.AppGroup) bool {
	selected, total := categorySelectionCounts(groups)
	return total > 0 && selected == total
}

func setCategorySelected(groups []cleaner.AppGroup, selected bool) {
	for _, appGroup := range groups {
		for _, group := range appGroup.Items {
			group.On = selected
		}
	}
}

func expandIcon(expanded bool) fyne.Resource {
	if expanded {
		return theme.MenuDropDownIcon()
	}
	return theme.MenuExpandIcon()
}

func cacheGroupDetails(texts *uiText, group *cleaner.Group) string {
	var b strings.Builder
	fmt.Fprintf(&b, "App: %s\nTarget: %s\nEstimated size: %s\n\nPaths:\n", group.App, group.Label, cleaner.HumanBytes(group.Bytes))
	if len(group.Paths) == 0 {
		fmt.Fprintf(&b, "- %s\n", texts.ResultNoMatchingPaths)
	}
	for _, path := range group.Paths {
		fmt.Fprintf(&b, "- %s\n", path)
	}
	if len(group.Errs) > 0 {
		fmt.Fprintf(&b, "\n%s\n", texts.ResultScanIssues)
		for _, err := range group.Errs {
			fmt.Fprintf(&b, "- %s\n", err)
		}
	}
	return b.String()
}

func resultGroupRow(texts *uiText, group *cleaner.GroupResult, w fyne.Window) fyne.CanvasObject {
	statusText := texts.ResultStatusOK
	if group.PathsFailed > 0 {
		statusText = texts.FailedCount(group.PathsFailed)
	} else if group.PathsAttempted == 0 {
		statusText = texts.ResultStatusSkipped
	}
	detailButton := widget.NewButtonWithIcon(texts.ResultDetails, theme.InfoIcon(), func() {
		var b strings.Builder
		fmt.Fprintf(&b, "%s - %s\nEstimated size: %s\nPaths attempted: %d\nPaths failed: %d\n", group.App, group.Label, cleaner.HumanBytes(group.Bytes), group.PathsAttempted, group.PathsFailed)
		if len(group.Errors) > 0 {
			b.WriteString("\nErrors:\n")
			for _, err := range group.Errors {
				fmt.Fprintf(&b, "- %s: %s\n", err.Path, err.Error)
			}
		}
		dialog.ShowInformation(texts.DialogCleanupDetailsTitle, b.String(), w)
	})
	return container.NewVBox(
		container.NewHBox(
			widget.NewLabelWithStyle(group.App, fyne.TextAlignLeading, fyne.TextStyle{Bold: true}),
			widget.NewLabel(group.Label),
			layout.NewSpacer(),
			widget.NewLabel(cleaner.HumanBytes(group.Bytes)),
			widget.NewLabel(statusText),
			detailButton,
		),
		widget.NewSeparator(),
	)
}

func showDryRunDialog(texts *uiText, plan cleaner.Plan, w fyne.Window) {
	if plan.Selected == 0 {
		dialog.ShowInformation(texts.ActionPreview, texts.DialogPreviewEmpty, w)
		return
	}

	rows := []fyne.CanvasObject{}
	for i := range plan.Groups {
		g := &plan.Groups[i]
		if !g.On {
			continue
		}
		sizeText := cleaner.HumanBytes(g.Bytes)
		if g.Bytes == 0 {
			sizeText = texts.NotFound
		}
		rows = append(rows, container.NewHBox(
			widget.NewLabelWithStyle(g.App, fyne.TextAlignLeading, fyne.TextStyle{Bold: true}),
			widget.NewLabel(g.Label),
			layout.NewSpacer(),
			widget.NewLabelWithStyle(sizeText, fyne.TextAlignTrailing, fyne.TextStyle{}),
		))
	}

	totalLabel := widget.NewLabelWithStyle(
		fmt.Sprintf("%s  |  est. %s", texts.ItemsCount(plan.Selected), cleaner.HumanBytes(plan.TotalBytes)),
		fyne.TextAlignLeading, fyne.TextStyle{Bold: true},
	)

	scroll := container.NewVScroll(container.NewVBox(rows...))
	scroll.SetMinSize(fyne.NewSize(640, 420))
	content := container.NewBorder(nil, totalLabel, nil, nil, scroll)
	dialog.NewCustom(texts.DialogPreviewTitle, texts.DialogClose, content, w).Show()
}
