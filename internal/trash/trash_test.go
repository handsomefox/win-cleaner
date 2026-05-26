//go:build windows

package trash

import (
	"os"
	"testing"

	"github.com/stretchr/testify/require"
)

func TestMoveToTrash(t *testing.T) {
	testFile, err := os.CreateTemp(t.TempDir(), "this_is_test_trash_new_for")
	require.NoError(t, err)
	defer os.Remove(testFile.Name())
	err = MoveToRecycleBin([]string{testFile.Name()})
	require.NoError(t, err)
}
