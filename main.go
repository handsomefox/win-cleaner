package main

import (
	"errors"
	"flag"
	"fmt"
	"log"
	"strings"

	"win-clear/internal/cleaner"
)

func main() {
	log.SetFlags(0)

	var (
		flagApply           = flag.Bool("apply", false, "Perform deletion (default is dry-run)")
		flagInteractive     = flag.Bool("interactive", true, "Interactive selection (GUI when available)")
		flagOnly            = flag.String("only", "", "Only include apps (comma-separated). Example: \"Chrome,Edge,Discord\"")
		flagSkip            = flag.String("skip", "", "Skip apps (comma-separated). Example: \"VSCode,Firefox\"")
		flagList            = flag.Bool("list", false, "List known apps and exit")
		flagSkipShaderCache = flag.Bool("skip-shader", false, "Skips the deletion of shader caches")
	)
	flag.Parse()

	opts := cleaner.Options{
		DryRun:      !*flagApply,
		Interactive: *flagInteractive,
	}

	if *flagList {
		reg := cleaner.BuildRegistry(cleaner.RegistryConfig{
			SkipShaderCache: *flagSkipShaderCache,
		})
		fmt.Println("Known applications:")
		for _, app := range reg.Apps() {
			fmt.Printf("- %s\n", app)
		}
		return
	}

	only := parseCSV(*flagOnly)
	skip := parseCSV(*flagSkip)

	reg := cleaner.BuildRegistry(cleaner.RegistryConfig{
		SkipShaderCache: *flagSkipShaderCache,
	})
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

	if opts.Interactive {
		if err := cleaner.RunGUI(reg, opts); err != nil {
			if errors.Is(err, cleaner.ErrCancelled) {
				fmt.Println("Cancelled.")
				return
			}
			log.Printf("Cleanup finished with errors: %v", err)
			return
		}
		return
	}

	plan, err := cleaner.BuildPlan(reg)
	if err != nil {
		log.Fatalf("Failed to build plan: %v", err)
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

	result, err := cleaner.ExecuteWithResult(plan, opts, nil)
	if !opts.DryRun {
		if _, statErr := cleaner.WriteStats(&result); statErr != nil {
			log.Printf("Failed to write stats: %v", statErr)
		}
	}
	if err != nil {
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
