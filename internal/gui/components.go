package gui

import (
	"fmt"
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

func headerBar(title fyne.CanvasObject, task *widget.Label, selectionChip, savingsChip fyne.CanvasObject, action *widget.Button) fyne.CanvasObject {
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

func labeledEntryRow(label string, entry *widget.Entry) fyne.CanvasObject {
	return container.NewBorder(nil, nil, widget.NewLabelWithStyle(label, fyne.TextAlignLeading, fyne.TextStyle{Bold: true}), nil, entry)
}

func controlsPanel(searchRow, actions fyne.CanvasObject) fyne.CanvasObject {
	return container.NewPadded(container.NewVBox(searchRow, actions))
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

func newCleanupProgressPanel(title, subtitle, selectedText, scopeText string) *cleanupProgressPanel {
	progress := widget.NewProgressBar()
	percent := widget.NewLabelWithStyle("0%", fyne.TextAlignTrailing, fyne.TextStyle{Bold: true})
	percent.SizeName = theme.SizeNameHeadingText
	percent.Importance = widget.HighImportance
	count := widget.NewLabelWithStyle("0 / 0", fyne.TextAlignLeading, fyne.TextStyle{Bold: true})
	current := widget.NewLabel("Preparing...")
	current.Wrapping = fyne.TextWrapWord
	destination := widget.NewLabel("Recycle Bin")

	status := container.NewBorder(
		nil, nil,
		container.NewHBox(widget.NewIcon(theme.DeleteIcon()), container.NewVBox(
			widget.NewLabelWithStyle("Moving selected items", fyne.TextAlignLeading, fyne.TextStyle{Bold: true}),
			current,
		)),
		percent,
		nil,
	)
	stats := container.NewGridWithColumns(3,
		progressStat(theme.CheckButtonCheckedIcon(), "Selected", selectedText),
		progressStat(theme.StorageIcon(), "Scope", scopeText),
		progressStat(theme.DeleteIcon(), "Destination", "Recycle Bin"),
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

func cacheListPanel(texts *uiText, list fyne.CanvasObject) fyne.CanvasObject {
	return surfaceCard(list)
}

func cacheCategorySection(category cacheCategoryView, expanded map[string]bool, appRows []fyne.CanvasObject, toggleSelected func()) fyne.CanvasObject {
	appsBox := container.NewVBox(appRows...)
	if !expanded[category.Name] {
		appsBox.Hide()
	}

	selectBtn := widget.NewButtonWithIcon("", cacheCategorySelectionIcon(category.Groups), toggleSelected)
	selectBtn.Importance = widget.LowImportance

	var expandBtn *widget.Button
	expandBtn = widget.NewButtonWithIcon("", expandIcon(expanded[category.Name]), func() {
		expanded[category.Name] = !expanded[category.Name]
		if expanded[category.Name] {
			appsBox.Show()
		} else {
			appsBox.Hide()
		}
		expandBtn.SetIcon(expandIcon(expanded[category.Name]))
	})
	expandBtn.Importance = widget.LowImportance

	nameLabel := widget.NewLabelWithStyle(category.Name, fyne.TextAlignLeading, fyne.TextStyle{Bold: true})
	countLabel := widget.NewLabel(fmt.Sprintf("%d apps", len(category.Groups)))
	sizeLabel := widget.NewLabelWithStyle(cleaner.HumanBytes(category.Bytes), fyne.TextAlignTrailing, fyne.TextStyle{Bold: true})
	header := container.NewPadded(container.NewBorder(
		nil, nil,
		container.NewHBox(expandBtn, selectBtn, widget.NewIcon(theme.FolderIcon()), nameLabel, countLabel),
		sizeLabel,
		nil,
	))
	return container.NewVBox(header, appsBox)
}

func cacheAppSection(texts *uiText, appName string, groups []*cleaner.Group, expanded map[string]bool, w fyne.Window, changed func()) fyne.CanvasObject {
	appSelect := widget.NewButtonWithIcon("", cacheAppSelectionIcon(groups), nil)
	appSelect.Importance = widget.LowImportance
	itemChecks := make([]*widget.Check, len(groups))
	itemRows := make([]fyne.CanvasObject, 0, len(groups))
	onCount := 0
	var total uint64

	for i, group := range groups {
		grp := group
		total += grp.Bytes
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

		sizeText := cleaner.HumanBytes(grp.Bytes)
		if grp.Bytes == 0 {
			sizeText = texts.NotFound
		}
		details := texts.ResultDetails
		if len(grp.Errs) > 0 {
			details = fmt.Sprintf("%s (%d issues)", texts.ResultDetails, len(grp.Errs))
		}
		detailButton := widget.NewButtonWithIcon(details, theme.InfoIcon(), func() {
			dialog.ShowInformation(grp.App+" - "+grp.Label, cacheGroupDetails(texts, grp), w)
		})
		itemRows = append(itemRows, cacheGroupRow(grp.Label, sizeText, chk, detailButton))
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
	isExpanded := expanded[appName]
	if !isExpanded {
		itemsBox.Hide()
	}
	var expandBtn *widget.Button
	expandBtn = widget.NewButtonWithIcon("", expandIcon(isExpanded), func() {
		expanded[appName] = !expanded[appName]
		if expanded[appName] {
			itemsBox.Show()
		} else {
			itemsBox.Hide()
		}
		expandBtn.SetIcon(expandIcon(expanded[appName]))
	})
	expandBtn.Importance = widget.LowImportance

	header := cacheAppHeader(appName, fmt.Sprintf("%d/%d selected", onCount, len(groups)), cleaner.HumanBytes(total), expandBtn, appSelect)
	return container.NewVBox(header, itemsBox, widget.NewSeparator())
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

func cacheAppHeader(appName, selectedText, sizeText string, expandBtn, appCheck fyne.CanvasObject) fyne.CanvasObject {
	nameLabel := widget.NewLabelWithStyle(appName, fyne.TextAlignLeading, fyne.TextStyle{Bold: true})
	selectedLabel := widget.NewLabel(selectedText)
	sizeLabel := widget.NewLabelWithStyle(sizeText, fyne.TextAlignTrailing, fyne.TextStyle{Bold: true})
	return container.NewPadded(container.NewBorder(
		nil, nil,
		container.NewHBox(expandBtn, appCheck, nameLabel, selectedLabel),
		sizeLabel,
		nil,
	))
}

func cacheGroupRow(label, sizeText string, chk *widget.Check, details *widget.Button) fyne.CanvasObject {
	sizeLabel := widget.NewLabelWithStyle(sizeText, fyne.TextAlignTrailing, fyne.TextStyle{})
	if sizeText == "not found" {
		sizeLabel.TextStyle = fyne.TextStyle{Italic: true}
	}
	pathLabel := widget.NewLabel(label)
	return container.NewPadded(container.NewBorder(
		nil, nil,
		container.NewHBox(widget.NewLabel("      "), chk),
		container.NewHBox(sizeLabel, details),
		pathLabel,
	))
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
		b.WriteString("\n" + texts.ResultScanIssues + "\n")
		for _, err := range group.Errs {
			fmt.Fprintf(&b, "- %s\n", err)
		}
	}
	return b.String()
}

func resultGroupRow(texts *uiText, group *cleaner.GroupResult, w fyne.Window) fyne.CanvasObject {
	statusText := texts.ResultStatusOK
	if group.PathsFailed > 0 {
		statusText = fmt.Sprintf("%d failed", group.PathsFailed)
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
		fmt.Sprintf("%d groups  |  est. %s", plan.Selected, cleaner.HumanBytes(plan.TotalBytes)),
		fyne.TextAlignLeading, fyne.TextStyle{Bold: true},
	)

	scroll := container.NewVScroll(container.NewVBox(rows...))
	scroll.SetMinSize(fyne.NewSize(640, 420))
	content := container.NewBorder(nil, totalLabel, nil, nil, scroll)
	dialog.NewCustom(texts.DialogPreviewTitle, texts.DialogClose, content, w).Show()
}
