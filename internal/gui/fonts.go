package gui

import (
	_ "embed"

	"fyne.io/fyne/v2"
)

// Inter (SIL Open Font License 1.1, see fonts/OFL.txt) is embedded as the UI
// typeface. Fyne selects a distinct resource per text style, so static weights
// are required for bold to actually render bold.

//go:embed fonts/Inter-Regular.ttf
var interRegularTTF []byte

//go:embed fonts/Inter-Bold.ttf
var interBoldTTF []byte

//go:embed fonts/Inter-Italic.ttf
var interItalicTTF []byte

//go:embed fonts/Inter-BoldItalic.ttf
var interBoldItalicTTF []byte

var (
	interRegular    = fyne.NewStaticResource("Inter-Regular.ttf", interRegularTTF)
	interBold       = fyne.NewStaticResource("Inter-Bold.ttf", interBoldTTF)
	interItalic     = fyne.NewStaticResource("Inter-Italic.ttf", interItalicTTF)
	interBoldItalic = fyne.NewStaticResource("Inter-BoldItalic.ttf", interBoldItalicTTF)
)

// interFont returns the embedded Inter resource matching the requested style.
func interFont(style fyne.TextStyle) fyne.Resource {
	switch {
	case style.Bold && style.Italic:
		return interBoldItalic
	case style.Bold:
		return interBold
	case style.Italic:
		return interItalic
	default:
		return interRegular
	}
}
