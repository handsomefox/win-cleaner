package gui

import (
	"fmt"

	"fyne.io/fyne/v2"
	"fyne.io/fyne/v2/widget"

	"win-clear/internal/cleaner"
)

// appHeader returns the shared top-of-window branding label.
func appHeader() *widget.Label {
	return widget.NewLabelWithStyle("win-cleaner", fyne.TextAlignLeading, fyne.TextStyle{Bold: true})
}

func recomputePlanTotals(plan *cleaner.Plan) {
	var total uint64
	var selected int
	for _, group := range plan.Groups {
		if group.On {
			selected++
			total += group.Bytes
		}
	}
	plan.TotalBytes = total
	plan.Selected = selected
}

func countSelectedEmptyFolders(plan *cleaner.EmptyFolderPlan) int {
	var selected int
	for _, folder := range plan.Folders {
		if folder.On {
			selected++
		}
	}
	return selected
}

func selectedEmptySummary(plan *cleaner.EmptyFolderPlan) string {
	return fmt.Sprintf("%d folders selected", countSelectedEmptyFolders(plan))
}
