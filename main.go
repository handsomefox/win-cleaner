package main

import (
	"errors"
	"flag"
	"fmt"
	"log"

	"win-clear/internal/cleaner"
)

func main() {
	log.SetFlags(0)

	var (
		flagApply       = flag.Bool("apply", false, "Perform deletion (default is dry-run)")
		flagInteractive = flag.Bool("interactive", true, "Interactive selection (GUI when available)")
		flagList        = flag.Bool("list", false, "List known apps and exit")
	)
	flag.Parse()

	opts := cleaner.Options{
		DryRun:      !*flagApply,
		Interactive: *flagInteractive,
	}

	reg := cleaner.BuildRegistry()

	if *flagList {
		fmt.Println("Known applications:")
		for _, app := range reg.Apps() {
			fmt.Printf("- %s\n", app)
		}
		return
	}

	if opts.Interactive {
		if err := cleaner.RunGUI(reg, opts); err != nil {
			if errors.Is(err, cleaner.ErrCancelled) {
				fmt.Println("Cancelled.")
				return
			}
			log.Printf("Cleanup finished with errors: %v", err)
		}
		return
	}

	plan, err := cleaner.BuildPlan(reg)
	if err != nil {
		log.Fatalf("Failed to build plan: %v", err)
	}

	fmt.Println(cleaner.ReportPlan(plan))

	if opts.DryRun {
		fmt.Println("Dry-run: no deletions will be performed. Use -apply to delete.")
		return
	}

	ok, err := cleaner.ConfirmProceed(plan, opts)
	if err != nil {
		log.Fatalf("Prompt failed: %v", err)
	}
	if !ok {
		fmt.Println("Cancelled.")
		return
	}

	result, err := cleaner.ExecuteWithResult(plan, opts, nil)
	if _, statErr := cleaner.WriteStats(&result); statErr != nil {
		log.Printf("Failed to write stats: %v", statErr)
	}
	if err != nil {
		log.Fatalf("Cleanup failed: %v", err)
	}

	fmt.Println("Cleanup complete.")
}
