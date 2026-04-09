# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Overview

`win-cleaner` is a Windows cache-cleaning tool with a Fyne GUI. It scans known cache locations for common apps, presents a 2-level grouped selection UI (App > Items), and moves selected paths to the Recycle Bin (never permanent deletion). Dry-run is the default.

Go module name: `win-clear` (see `go.mod`).

## Commands

```sh
make build              # cross-compile win-cleaner.exe via fyne-cross (GOOS=windows GOARCH=amd64)
go run . -list          # list known apps and exit
go run .                # dry-run scan (no deletions)
go run . -apply         # perform deletions after confirmation (CLI mode)
go test ./...           # run all tests (trash tests require Windows)
```

**Always use `make build` to verify compilation — do not use `go build ./...`.**

After all implementation tasks are complete, run in this order:
1. `gofumpt -l -w .`
2. `golangci-lint run --fix ./...`
3. `go vet ./...`
4. `deadcode ./...`
5. `betteralign -apply ./...`
6. `govulncheck ./...`

**Note:** `BuildPlan` and actual deletions use `runtime.GOOS != "windows"` guards and will error on non-Windows platforms. Tests in `internal/trash` require Windows for full coverage of `MoveToRecycleBin`.

## Code style preferences

- No `nolint` directives.
- Only fix `errcheck` lints if they are significant (e.g. ignore `fmt.Fprintf` to `strings.Builder`, `w.Flush()`, `os.Remove` in defers).
- No section divider comments inside functions (e.g. `// --- Section ---`).
- No em-dashes in strings; use hyphens instead.
- Commit after each logical task/feature is complete.
- No backwards-compatibility shims or dead code — delete unused functions immediately.

## Architecture

### Data flow

1. **`BuildRegistry`** (`internal/cleaner/registry.go`) — constructs `[]Item` from Windows environment variables (`LOCALAPPDATA`, `APPDATA`, `PROGRAMDATA`, `USERPROFILE`, `SystemRoot`). Returns an empty registry if env vars are missing (non-Windows). Items are defined as a single `[]Item` literal grouped by category in comments.

2. **`BuildPlan`** (`internal/cleaner/scan.go`) — expands globs, walks directories to measure sizes, applies `isSafePath` guards, returns a `Plan` (sorted by App then Label).

3. **`Plan.ByApp()`** (`internal/cleaner/scan.go`) — groups `Plan.Groups` into `[]AppGroup` for the 2-level GUI view.

4. **`RunGUI`** (`internal/cleaner/gui.go`) — Fyne three-screen flow: Scan → Select → Delete (or Dry-run dialog). `showSelect` renders app-level header checkboxes (select all in app) with indented item rows underneath. Stats button opens `showStatsDialog`.

5. **`ExecuteWithResult`** (`internal/cleaner/scan.go`) — iterates selected groups, calls `DeletePathSmart` per path, accumulates `ExecResult`.

6. **`DeletePathSmart`** (`internal/cleaner/delete.go`) — directories with >500 children are recycled whole (fast path); otherwise children are batched in chunks of 64. Files/links go directly. Uses `internal/trash.MoveToRecycleBin`.

7. **`WriteStats` / `LoadStats`** (`internal/cleaner/stats.go`) — persists `ExecResult` as JSON to `~/.win-cleaner/stats/` (one file per run).

### Key types

- `Registry` / `Item` — the static catalog of what to clean and where.
- `Plan` / `Group` / `AppGroup` — the runtime snapshot: resolved paths + measured sizes + selection state. `AppGroup` is the 2-level grouping used by the GUI.
- `ExecResult` / `GroupResult` — JSON-serializable cleanup outcome stored in stats.

### Safety

- `isSafePath` requires a path to be *under* (not equal to) one of the four safe roots. Nothing outside those roots can be deleted.
- All deletions go through the Windows Recycle Bin (`SHFileOperationW` via `internal/trash/trash_windows.go`). Permanent deletion is never used.

### GUI

Built with Fyne v2. All goroutine→UI updates use `a.Driver().DoFromGoroutine`. The select screen uses `container.NewVScroll(container.NewVBox(...))` with per-app sections from `plan.ByApp()`.
