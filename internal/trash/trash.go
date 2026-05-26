//go:build !windows

// Package trash wraps the Windows Recycle Bin API (SHFileOperationW).
// This file is the non-Windows stub.
package trash

import "errors"

var ErrUnsupportedPlatform = errors.New("recycle bin is only supported on Windows")

func MoveToRecycleBin(paths []string) error {
	return ErrUnsupportedPlatform
}
