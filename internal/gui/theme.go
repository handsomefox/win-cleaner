package gui

import (
	"image/color"

	"fyne.io/fyne/v2"
	"fyne.io/fyne/v2/theme"
)

// Refined dark palette with an indigo accent. These are the single source of
// truth for the theme: Color() returns them, and custom canvas surfaces
// (surfaceCard, chip, ...) reference the same vars so everything stays in sync.
var (
	colBackground    = color.NRGBA{R: 0x0f, G: 0x11, B: 0x15, A: 0xff} // deep app background
	colSurface       = color.NRGBA{R: 0x18, G: 0x1b, B: 0x22, A: 0xff} // cards / header / menus
	colSurfaceRaised = color.NRGBA{R: 0x1f, G: 0x23, B: 0x2c, A: 0xff} // buttons / chips
	colSurfaceSunken = color.NRGBA{R: 0x16, G: 0x19, B: 0x22, A: 0xff} // inputs
	colDisabledBtn   = color.NRGBA{R: 0x17, G: 0x1a, B: 0x20, A: 0xff}
	colBorder        = color.NRGBA{R: 0x2a, G: 0x2f, B: 0x3a, A: 0xff}
	colHover         = color.NRGBA{R: 0x25, G: 0x2b, B: 0x36, A: 0xff}
	colPressed       = color.NRGBA{R: 0x2f, G: 0x36, B: 0x43, A: 0xff}
	colAccent        = color.NRGBA{R: 0x63, G: 0x66, B: 0xf1, A: 0xff} // indigo
	colSelection     = color.NRGBA{R: 0x63, G: 0x66, B: 0xf1, A: 0x55} // translucent indigo
	colRowAlt        = color.NRGBA{R: 0xff, G: 0xff, B: 0xff, A: 0x0c} // zebra-stripe overlay
	colTransparent   = color.NRGBA{}
	colOnAccent      = color.NRGBA{R: 0xff, G: 0xff, B: 0xff, A: 0xff}
	colText          = color.NRGBA{R: 0xe6, G: 0xe9, B: 0xef, A: 0xff}
	colMuted         = color.NRGBA{R: 0x8b, G: 0x93, B: 0xa3, A: 0xff}
	colScrollBar     = color.NRGBA{R: 0x3a, G: 0x41, B: 0x50, A: 0xcc}
	colScrollBarBg   = color.NRGBA{R: 0x16, G: 0x19, B: 0x22, A: 0x66}
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
		return colBackground
	case theme.ColorNameButton:
		return colSurfaceRaised
	case theme.ColorNameDisabledButton:
		return colDisabledBtn
	case theme.ColorNameHeaderBackground, theme.ColorNameMenuBackground, theme.ColorNameOverlayBackground:
		return colSurface
	case theme.ColorNameInputBackground:
		return colSurfaceSunken
	case theme.ColorNameInputBorder, theme.ColorNameSeparator:
		return colBorder
	case theme.ColorNameHover:
		return colHover
	case theme.ColorNamePressed:
		return colPressed
	case theme.ColorNamePrimary, theme.ColorNameFocus, theme.ColorNameHyperlink:
		return colAccent
	case theme.ColorNameSelection:
		return colSelection
	case theme.ColorNameForeground:
		return colText
	case theme.ColorNameForegroundOnPrimary:
		return colOnAccent
	case theme.ColorNamePlaceHolder, theme.ColorNameDisabled:
		return colMuted
	case theme.ColorNameScrollBar:
		return colScrollBar
	case theme.ColorNameScrollBarBackground:
		return colScrollBarBg
	default:
		return t.base.Color(name, theme.VariantDark)
	}
}

func (t *winCleanerTheme) Font(style fyne.TextStyle) fyne.Resource {
	if style.Monospace || style.Symbol {
		return t.base.Font(style)
	}
	return interFont(style)
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
		return 6
	case theme.SizeNameInnerPadding:
		return 9
	case theme.SizeNameText:
		return 15
	case theme.SizeNameHeadingText:
		return 22
	case theme.SizeNameSubHeadingText:
		return 18
	case theme.SizeNameCaptionText:
		return 12
	default:
		return t.base.Size(name)
	}
}
