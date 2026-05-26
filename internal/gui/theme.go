package gui

import (
	"image/color"

	"fyne.io/fyne/v2"
	"fyne.io/fyne/v2/theme"
)

type winCleanerTheme struct {
	base fyne.Theme
}

func newWinCleanerTheme() fyne.Theme {
	return &winCleanerTheme{base: theme.DefaultTheme()}
}

func (t *winCleanerTheme) Color(name fyne.ThemeColorName, variant fyne.ThemeVariant) color.Color {
	switch name {
	case theme.ColorNameBackground:
		return color.NRGBA{R: 0x18, G: 0x18, B: 0x1b, A: 0xff}
	case theme.ColorNameButton:
		return color.NRGBA{R: 0x2d, G: 0x2f, B: 0x35, A: 0xff}
	case theme.ColorNameDisabledButton:
		return color.NRGBA{R: 0x23, G: 0x24, B: 0x28, A: 0xff}
	case theme.ColorNameHeaderBackground, theme.ColorNameMenuBackground, theme.ColorNameOverlayBackground:
		return color.NRGBA{R: 0x20, G: 0x21, B: 0x26, A: 0xff}
	case theme.ColorNameInputBackground:
		return color.NRGBA{R: 0x22, G: 0x23, B: 0x28, A: 0xff}
	case theme.ColorNameInputBorder, theme.ColorNameSeparator:
		return color.NRGBA{R: 0x3b, G: 0x3d, B: 0x45, A: 0xff}
	case theme.ColorNameHover:
		return color.NRGBA{R: 0x36, G: 0x38, B: 0x40, A: 0xff}
	case theme.ColorNamePressed:
		return color.NRGBA{R: 0x43, G: 0x45, B: 0x4e, A: 0xff}
	case theme.ColorNamePrimary, theme.ColorNameFocus, theme.ColorNameHyperlink:
		return color.NRGBA{R: 0x4f, G: 0xc3, B: 0xa6, A: 0xff}
	case theme.ColorNameSelection:
		return color.NRGBA{R: 0x3a, G: 0x6f, B: 0x66, A: 0xff}
	case theme.ColorNameForegroundOnPrimary:
		return color.NRGBA{R: 0x0d, G: 0x12, B: 0x13, A: 0xff}
	case theme.ColorNameScrollBar:
		return color.NRGBA{R: 0x68, G: 0x6b, B: 0x75, A: 0xdd}
	case theme.ColorNameScrollBarBackground:
		return color.NRGBA{R: 0x26, G: 0x27, B: 0x2d, A: 0x99}
	default:
		return t.base.Color(name, theme.VariantDark)
	}
}

func (t *winCleanerTheme) Font(style fyne.TextStyle) fyne.Resource {
	return t.base.Font(style)
}

func (t *winCleanerTheme) Icon(name fyne.ThemeIconName) fyne.Resource {
	return t.base.Icon(name)
}

func (t *winCleanerTheme) Size(name fyne.ThemeSizeName) float32 {
	switch name {
	case theme.SizeNameInputRadius:
		return 12
	case theme.SizeNameSelectionRadius:
		return 9
	case theme.SizeNameScrollBarRadius:
		return 8
	case theme.SizeNameWindowButtonRadius:
		return 10
	case theme.SizeNamePadding:
		return 4
	case theme.SizeNameInnerPadding:
		return 7
	case theme.SizeNameText:
		return 15
	case theme.SizeNameHeadingText:
		return 23
	case theme.SizeNameSubHeadingText:
		return 18
	case theme.SizeNameCaptionText:
		return 12
	default:
		return t.base.Size(name)
	}
}
