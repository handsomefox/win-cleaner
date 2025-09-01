package trash

import (
	"os"
	"testing"
)

func TestMoveToTrash(t *testing.T) {
	testFile, err := os.CreateTemp(t.TempDir(), "this_is_test_trash_new_for")
	if err != nil {
		t.Fatalf("unable to create temp file: %v", err)
	}
	defer os.Remove(testFile.Name())
	err = MoveToRecycleBin([]string{testFile.Name()})
	if err != nil {
		t.Fatalf("failed to move file to trash: %v", err)
	}
}
