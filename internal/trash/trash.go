//go:build !windows
// +build !windows

package trash

func MoveToRecycleBin(paths []string) error {
	panic("unimplemented")
}
