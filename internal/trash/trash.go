//go:build !windows

// Package trash wraps the Windows Recycle Bin API (SHFileOperationW).
// This file is the non-Windows stub.
package trash

func MoveToRecycleBin(paths []string) error {
	panic("unimplemented")
}
