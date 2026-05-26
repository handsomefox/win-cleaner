// Package gui implements the Fyne interface for win-cleaner.
package gui

import (
	"errors"
	"runtime"
	"sync/atomic"

	"fyne.io/fyne/v2"
	"fyne.io/fyne/v2/app"

	"win-clear/internal/cleaner"
)

func Run(reg cleaner.Registry, opts cleaner.Options) error {
	if runtime.GOOS != "windows" {
		return cleaner.ErrGUIUnavailable
	}

	texts := englishText()
	a := app.NewWithID("win-cleaner")
	a.Settings().SetTheme(newWinCleanerTheme())
	w := a.NewWindow(texts.AppTitle)
	w.Resize(fyne.NewSize(1320, 860))
	w.CenterOnScreen()

	result := cleaner.ErrCancelled
	var closing atomic.Bool
	safeClose := func(err error) {
		if closing.Swap(true) {
			return
		}
		result = err
		w.Close()
	}

	w.SetCloseIntercept(func() {
		safeClose(cleaner.ErrCancelled)
	})

	ws := newWorkspace(a, w, reg, opts, texts, safeClose)
	ws.showCacheScan()
	ws.showEmptyRootSelect()
	ws.showHistory()

	w.ShowAndRun()
	if errors.Is(result, cleaner.ErrCancelled) {
		return cleaner.ErrCancelled
	}
	return result
}
