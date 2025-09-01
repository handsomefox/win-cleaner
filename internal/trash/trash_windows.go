//go:build windows

package trash

import (
	"fmt"
	"path/filepath"
	"syscall"
	"unsafe"
)

/*
We use SHFileOperationW with FO_DELETE and FOF_ALLOWUNDO to move to Recycle Bin.

Key requirements:
- pFrom must be a double-NUL-terminated UTF-16 list (PCZZWSTR). We must build []uint16 manually.
- Paths must be absolute. Backslashes are fine with filepath.Clean on Windows.
- SHFileOperationW returns 0 on success; nonzero means failure.
*/

const (
	FO_DELETE          = 3
	FOF_MULTIDESTFILES = 0x0001
	FOF_NOCONFIRMATION = 0x0010
	FOF_SILENT         = 0x0004
	FOF_ALLOWUNDO      = 0x0040
	FOF_NOERRORUI      = 0x0400
)

type shFileOpStructW struct {
	Hwnd                  uintptr
	WFunc                 uint32
	PFrom                 *uint16 // PCZZWSTR
	PTo                   *uint16
	FFlags                uint16
	FAnyOperationsAborted int32
	HNameMappings         uintptr
	LpszProgressTitle     *uint16
}

var (
	modShell32           = syscall.NewLazyDLL("shell32.dll")
	procSHFileOperationW = modShell32.NewProc("SHFileOperationW")
)

// MoveToRecycleBin moves one or more absolute paths to the Recycle Bin in a single call.
func MoveToRecycleBin(paths []string) error {
	if len(paths) == 0 {
		return nil
	}

	// Build a double-NUL-terminated UTF-16 list.
	// Example layout: "C:\foo\0C:\bar\0\0"
	list := make([]uint16, 0, 256)
	for _, p := range paths {
		if p == "" {
			continue
		}
		abs, err := filepath.Abs(p)
		if err != nil {
			return err
		}
		u := syscall.StringToUTF16(abs) // includes trailing NUL
		list = append(list, u...)       // append path + NUL
	}
	if len(list) == 0 {
		return nil
	}
	// Ensure final double NUL: StringToUTF16 added one; add one more now.
	list = append(list, 0)

	op := &shFileOpStructW{
		Hwnd:   0,
		WFunc:  FO_DELETE,
		PFrom:  &list[0],
		PTo:    nil,
		FFlags: FOF_ALLOWUNDO | FOF_NOCONFIRMATION | FOF_NOERRORUI | FOF_SILENT,
	}

	ret, _, _ := procSHFileOperationW.Call(uintptr(unsafe.Pointer(op)))
	if ret != 0 {
		return fmt.Errorf("SHFileOperationW failed, code=%d", ret)
	}
	if op.FAnyOperationsAborted != 0 {
		// Treat as a failure to be safe-first
		return fmt.Errorf("operation aborted by shell")
	}
	return nil
}
