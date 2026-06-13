package gui

import (
	_ "embed"
	"net/url"

	"fyne.io/fyne/v2"
	"fyne.io/fyne/v2/container"
	"fyne.io/fyne/v2/dialog"
	"fyne.io/fyne/v2/theme"
	"fyne.io/fyne/v2/widget"
)

//go:embed icon.png
var iconPNG []byte

// appIcon is the embedded application icon used for the window and About dialog.
var appIcon = fyne.NewStaticResource("icon.png", iconPNG)

// showAbout presents the application's About dialog.
func showAbout(w fyne.Window, texts *uiText) {
	title := widget.NewLabelWithStyle(texts.AppTitle, fyne.TextAlignLeading, fyne.TextStyle{Bold: true})
	title.SizeName = theme.SizeNameHeadingText

	version := widget.NewLabel(texts.AboutVersionLabel + " " + appVersion)
	version.Importance = widget.LowImportance
	fonts := widget.NewLabel(texts.AboutFonts)
	fonts.Importance = widget.LowImportance

	var repo fyne.CanvasObject
	if u, err := url.Parse(texts.AboutRepoURL); err == nil {
		repo = widget.NewHyperlink(texts.AboutRepoLabel, u)
	} else {
		repo = widget.NewLabel(texts.AboutRepoURL)
	}

	icon := widget.NewIcon(appIcon)
	details := container.NewVBox(title, version, fonts, repo)
	content := container.NewBorder(nil, nil, container.NewPadded(icon), nil, details)
	dialog.ShowCustom(texts.MenuAbout, texts.DialogClose, content, w)
}
