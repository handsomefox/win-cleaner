package gui

import (
	_ "embed"

	"fyne.io/fyne/v2"
)

// Category glyphs from Lucide (ISC, see icons/LICENSE). Their stroke color is
// baked to the muted theme tone, so they embed as plain static resources.

//go:embed icons/globe.svg
var iconBrowsersSVG []byte

//go:embed icons/message-circle.svg
var iconChatSVG []byte

//go:embed icons/code.svg
var iconDevSVG []byte

//go:embed icons/gamepad-2.svg
var iconGamingSVG []byte

//go:embed icons/music.svg
var iconMediaSVG []byte

//go:embed icons/settings.svg
var iconSystemSVG []byte

//go:embed icons/palette.svg
var iconCreativeSVG []byte

//go:embed icons/box.svg
var iconOtherSVG []byte

var (
	iconBrowsers = fyne.NewStaticResource("globe.svg", iconBrowsersSVG)
	iconChat     = fyne.NewStaticResource("message-circle.svg", iconChatSVG)
	iconDev      = fyne.NewStaticResource("code.svg", iconDevSVG)
	iconGaming   = fyne.NewStaticResource("gamepad-2.svg", iconGamingSVG)
	iconMedia    = fyne.NewStaticResource("music.svg", iconMediaSVG)
	iconSystem   = fyne.NewStaticResource("settings.svg", iconSystemSVG)
	iconCreative = fyne.NewStaticResource("palette.svg", iconCreativeSVG)
	iconOther    = fyne.NewStaticResource("box.svg", iconOtherSVG)
)

// categoryIcon maps a localized category name to its glyph.
func categoryIcon(texts *uiText, name string) fyne.Resource {
	switch name {
	case texts.CacheCategoryBrowsers:
		return iconBrowsers
	case texts.CacheCategoryChat:
		return iconChat
	case texts.CacheCategoryDevelopment:
		return iconDev
	case texts.CacheCategoryGaming:
		return iconGaming
	case texts.CacheCategoryMedia:
		return iconMedia
	case texts.CacheCategorySystem:
		return iconSystem
	case texts.CacheCategoryCreative:
		return iconCreative
	default:
		return iconOther
	}
}
