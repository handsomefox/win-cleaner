package main

import (
	"errors"
	"fmt"
	"log"

	"win-clear/internal/cleaner"
)

func main() {
	log.SetFlags(0)

	opts := cleaner.Options{
		DryRun: true,
	}

	reg := cleaner.BuildRegistry()
	if err := cleaner.RunGUI(reg, opts); err != nil {
		if errors.Is(err, cleaner.ErrCancelled) {
			fmt.Println("Cancelled.")
			return
		}
		log.Printf("Cleanup finished with errors: %v", err)
	}
}
