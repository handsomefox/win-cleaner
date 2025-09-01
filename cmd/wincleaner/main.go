package main

import (
	"flag"
	"fmt"
	"log"
	"strings"

	"win-clear/internal/cleaner"
)

func main() {
	log.SetFlags(0)

	var (
		flagApply       = flag.Bool("apply", false, "Perform deletion (default is dry-run)")
		flagInteractive = flag.Bool("interactive", false, "Interactive selection (uncheck caches)")
		flagOnly        = flag.String("only", "", "Only include apps (comma-separated). Example: \"Chrome,Edge,Discord\"")
		flagSkip        = flag.String("skip", "", "Skip apps (comma-separated). Example: \"VSCode,Firefox\"")
		flagList        = flag.Bool("list", false, "List known apps and exit")
	)
	flag.Parse()

	opts := cleaner.Options{
		DryRun:      !*flagApply,
		Interactive: *flagInteractive,
	}

	if *flagList {
		reg := cleaner.BuildRegistry()
		fmt.Println("Known applications:")
		for _, app := range reg.Apps() {
			fmt.Printf("- %s\n", app)
		}
		return
	}

	only := parseCSV(*flagOnly)
	skip := parseCSV(*flagSkip)

	reg := cleaner.BuildRegistry()
	if len(only) > 0 {
		reg = reg.FilterInclude(only)
	}
	if len(skip) > 0 {
		reg = reg.FilterExclude(skip)
	}

	if len(reg.Items) == 0 {
		fmt.Println("No matching apps after filters. Nothing to do.")
		return
	}

	plan, err := cleaner.BuildPlan(reg)
	if err != nil {
		log.Fatalf("Failed to build plan: %v", err)
	}

	if opts.Interactive {
		err = cleaner.InteractiveSelect(&plan)
		if err != nil {
			log.Fatalf("Interactive selection failed: %v", err)
		}
	}

	report := cleaner.ReportPlan(plan)
	fmt.Println(report)

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

	if err := cleaner.Execute(plan, opts); err != nil {
		log.Fatalf("Cleanup failed: %v", err)
	}

	fmt.Println("Cleanup complete.")
}

func parseCSV(s string) []string {
	if strings.TrimSpace(s) == "" {
		return nil
	}
	parts := strings.Split(s, ",")
	var out []string
	for _, p := range parts {
		p = strings.TrimSpace(p)
		if p != "" {
			out = append(out, p)
		}
	}
	return out
}
