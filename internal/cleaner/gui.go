package cleaner

import (
	"errors"
	"fmt"
	"runtime"
	"strings"
	"sync/atomic"

	"fyne.io/fyne/v2"
	"fyne.io/fyne/v2/app"
	"fyne.io/fyne/v2/container"
	"fyne.io/fyne/v2/dialog"
	"fyne.io/fyne/v2/layout"
	"fyne.io/fyne/v2/theme"
	"fyne.io/fyne/v2/widget"
)

func RunGUI(reg Registry, opts Options) error {
	if runtime.GOOS != "windows" {
		return ErrGUIUnavailable
	}

	a := app.NewWithID("win-cleaner")
	w := a.NewWindow("win-cleaner")
	w.Resize(fyne.NewSize(980, 680))

	title := widget.NewLabelWithStyle("win-cleaner", fyne.TextAlignLeading, fyne.TextStyle{Bold: true})
	subtitle := widget.NewLabel("Windows cache cleaner")

	var result error = ErrCancelled
	var closing atomic.Bool
	safeClose := func(err error) {
		if closing.Swap(true) {
			return
		}
		result = err
		w.Close()
	}

	w.SetCloseIntercept(func() {
		safeClose(ErrCancelled)
	})

	showScan := func() {
		status := widget.NewLabel("Scanning cache locations...")
		progress := widget.NewProgressBar()
		progress.Min = 0
		progress.Max = 1
		progress.SetValue(0)

		content := container.NewBorder(
			container.NewVBox(title, subtitle),
			container.NewHBox(layout.NewSpacer(), widget.NewButtonWithIcon("Cancel", theme.CancelIcon(), func() {
				safeClose(ErrCancelled)
			})),
			nil,
			nil,
			container.NewVBox(status, progress),
		)
		w.SetContent(content)

		go func() {
			plan, err := BuildPlanWithProgress(reg, func(u ProgressUpdate) {
				if closing.Load() {
					return
				}
				a.Driver().DoFromGoroutine(func() {
					if u.Total > 0 {
						progress.SetValue(float64(u.Current) / float64(u.Total))
					}
					if u.Message != "" {
						status.SetText(fmt.Sprintf("Scanning (%d/%d): %s", u.Current, u.Total, u.Message))
					}
				}, false)
			})
			if closing.Load() {
				return
			}
			a.Driver().DoFromGoroutine(func() {
				if err != nil {
					dialog.ShowError(err, w)
					safeClose(err)
					return
				}
				showSelect(&plan, opts, w, a, title, subtitle, safeClose)
			}, false)
		}()
	}

	showScan()
	w.ShowAndRun()
	if errors.Is(result, ErrCancelled) {
		return ErrCancelled
	}
	return result
}

func showSelect(plan *Plan, opts Options, w fyne.Window, a fyne.App, title, subtitle *widget.Label, safeClose func(error)) {
	filterEntry := widget.NewEntry()
	filterEntry.SetPlaceHolder("Filter apps or labels")

	dryRun := opts.DryRun

	selectedLabel := widget.NewLabel("")
	savingsLabel := widget.NewLabel("")

	expanded := map[int]bool{}
	var listScroll *container.Scroll

	updateSummary := func() {
		recomputeTotals(plan)
		selectedLabel.SetText(fmt.Sprintf("Selected: %d", plan.Selected))
		savingsLabel.SetText("Est. savings: " + HumanBytes(plan.TotalBytes))
	}

	rebuildList := func(filter string) {
		filter = strings.ToLower(strings.TrimSpace(filter))
		items := make([]fyne.CanvasObject, 0, len(plan.Groups))
		for i := range plan.Groups {
			g := &plan.Groups[i]
			if filter != "" {
				hay := strings.ToLower(g.App + " " + g.Label)
				if !strings.Contains(hay, filter) {
					continue
				}
			}
			idx := i
			label := fmt.Sprintf("%s - %s (%s)", g.App, g.Label, HumanBytes(g.Bytes))
			check := widget.NewCheck("", func(checked bool) {
				plan.Groups[idx].On = checked
				updateSummary()
			})
			check.Checked = g.On
			check.Refresh()

			pathsHeader := widget.NewLabelWithStyle("Planned paths:", fyne.TextAlignLeading, fyne.TextStyle{Bold: true})
			var pathsText string
			if len(g.Paths) == 0 {
				pathsText = "No paths found."
			} else {
				pathsText = strings.Join(g.Paths, "\n")
			}
			pathsLabel := widget.NewLabel(pathsText)
			pathsLabel.Wrapping = fyne.TextWrapBreak
			pathsScroll := container.NewVScroll(pathsLabel)
			pathsScroll.SetMinSize(fyne.NewSize(0, 120))
			pathsBox := container.NewVBox(pathsHeader, pathsScroll)

			expand := widget.NewButton("Show paths", nil)
			if expanded[idx] {
				pathsBox.Show()
				expand.SetText("Hide paths")
			} else {
				pathsBox.Hide()
			}
			expand.OnTapped = func() {
				expanded[idx] = !expanded[idx]
				if expanded[idx] {
					pathsBox.Show()
					expand.SetText("Hide paths")
				} else {
					pathsBox.Hide()
					expand.SetText("Show paths")
				}
			}

			header := container.NewHBox(check, widget.NewLabel(label), layout.NewSpacer(), expand)
			row := container.NewVBox(header, pathsBox, widget.NewSeparator())
			items = append(items, row)
		}
		if len(items) == 0 {
			items = append(items, widget.NewLabel("No matches"))
		}
		content := container.NewVBox(items...)
		if listScroll == nil {
			listScroll = container.NewVScroll(content)
		} else {
			listScroll.Content = content
			listScroll.Refresh()
		}
	}

	filterEntry.OnChanged = func(s string) {
		rebuildList(s)
	}

	selectAll := widget.NewButtonWithIcon("Select All", theme.ContentAddIcon(), func() {
		for i := range plan.Groups {
			plan.Groups[i].On = true
		}
		rebuildList(filterEntry.Text)
		updateSummary()
	})
	selectNone := widget.NewButtonWithIcon("Select None", theme.ContentRemoveIcon(), func() {
		for i := range plan.Groups {
			plan.Groups[i].On = false
		}
		rebuildList(filterEntry.Text)
		updateSummary()
	})

	applyLabel := func() string {
		if dryRun {
			return "Close (Dry Run)"
		}
		return "Apply (Recycle Bin)"
	}

	apply := widget.NewButtonWithIcon(applyLabel(), theme.ConfirmIcon(), func() {
		updateSummary()
		if dryRun {
			safeClose(nil)
			return
		}
		if plan.Selected == 0 {
			dialog.ShowInformation("Nothing Selected", "Select at least one group to delete.", w)
			return
		}
		confirmText := fmt.Sprintf("Move %d selected groups to the Recycle Bin?\nEstimated savings: %s",
			plan.Selected, HumanBytes(plan.TotalBytes))
		dialog.NewConfirm("Confirm Cleanup", confirmText, func(ok bool) {
			if !ok {
				return
			}
			nextOpts := opts
			nextOpts.DryRun = dryRun
			showDelete(plan, nextOpts, w, a, title, subtitle, safeClose)
		}, w).Show()
	})

	updateApplyLabel := func() {
		apply.SetText(applyLabel())
	}

	dryRunToggle := widget.NewCheck("Dry run", func(checked bool) {
		dryRun = checked
		updateApplyLabel()
	})
	dryRunToggle.Checked = dryRun
	dryRunToggle.Refresh()

	cancel := widget.NewButtonWithIcon("Cancel", theme.CancelIcon(), func() {
		safeClose(ErrCancelled)
	})

	header := container.NewVBox(title, subtitle)
	filterRow := container.NewBorder(nil, nil, widget.NewLabelWithStyle("Filter:", fyne.TextAlignLeading, fyne.TextStyle{Bold: true}), nil, filterEntry)
	controls := container.NewHBox(selectAll, selectNone, dryRunToggle, layout.NewSpacer(), selectedLabel, savingsLabel)
	footer := container.NewHBox(layout.NewSpacer(), cancel, apply)

	rebuildList("")
	updateSummary()

	content := container.NewBorder(
		container.NewVBox(header, filterRow),
		container.NewVBox(controls, footer),
		nil,
		nil,
		listScroll,
	)
	w.SetContent(content)
}

func showDelete(plan *Plan, opts Options, w fyne.Window, a fyne.App, title, subtitle *widget.Label, safeClose func(error)) {
	status := widget.NewLabel("Deleting selected caches...")
	progress := widget.NewProgressBar()
	progress.Min = 0
	progress.Max = 1
	progress.SetValue(0)

	content := container.NewBorder(
		container.NewVBox(title, subtitle),
		nil,
		nil,
		nil,
		container.NewVBox(status, progress),
	)
	w.SetContent(content)

	go func() {
		err := ExecuteWithProgress(*plan, opts, func(u ProgressUpdate) {
			a.Driver().DoFromGoroutine(func() {
				if u.Total > 0 {
					progress.SetValue(float64(u.Current) / float64(u.Total))
				}
				if u.Message != "" {
					status.SetText(fmt.Sprintf("Deleting (%d/%d): %s", u.Current, u.Total, u.Message))
				}
			}, false)
		})
		a.Driver().DoFromGoroutine(func() {
			finalMsg := "Cleanup complete."
			if err != nil {
				finalMsg = "Cleanup finished with errors."
			}
			doneLabel := widget.NewLabel(finalMsg)
			closeBtn := widget.NewButtonWithIcon("Close", theme.ConfirmIcon(), func() {
				safeClose(err)
			})
			w.SetContent(container.NewBorder(
				container.NewVBox(title, subtitle),
				container.NewHBox(layout.NewSpacer(), closeBtn),
				nil,
				nil,
				container.NewVBox(doneLabel),
			))
		}, false)
	}()
}
