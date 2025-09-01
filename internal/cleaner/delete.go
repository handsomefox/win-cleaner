package cleaner

import (
	"path/filepath"

	"win-clear/internal/trash"
)

func moveToRecycleBin(path string) error {
	return trash.MoveToRecycleBin([]string{filepath.FromSlash(path)})
}
