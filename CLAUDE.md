# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Overview

`win-cleaner` is a Windows cache-cleaning tool with both a CLI and a Fyne GUI. It scans known cache locations for common apps, presents a selection UI, and moves selected paths to the Recycle Bin (never permanent deletion). Dry-run is the default.

Go module name: `win-clear` (see `go.mod`).

## Commands

```sh
make build              # cross-compile win-cleaner.exe via fyne-cross (GOOS=windows GOARCH=amd64)
go run . -list          # list known apps and exit
go run .                # dry-run scan (no deletions)
go run . -apply         # perform deletions after confirmation (CLI mode)
go test ./...           # run all tests
go test ./internal/trash/... -run TestName   # run a single test
```

**Note:** `BuildPlan` and actual deletions call `runtime.GOOS != "windows"` guards and will error on non-Windows platforms. Tests in `internal/trash` require Windows for full coverage of `MoveToRecycleBin`.

## Architecture

### Data flow

1. **`BuildRegistry`** (`internal/cleaner/registry.go`) — constructs the list of `Item`s (app name, label, static paths, glob patterns) from Windows environment variables (`LOCALAPPDATA`, `APPDATA`, `PROGRAMDATA`, `USERPROFILE`). Returns an empty registry if the env vars are missing (i.e., non-Windows).

2. **`BuildPlan`** (`internal/cleaner/scan.go`) — expands globs, walks directories to measure sizes, applies `isSafePath` guards, and returns a `Plan` (list of `Group`s with size estimates and default-on state). Sorted by App then Label.

3. **Selection** — either interactive CLI (`InteractiveSelect`) or Fyne GUI (`RunGUI` → `showSelect`).

4. **`ExecuteWithResult`** (`internal/cleaner/scan.go`) — iterates selected groups and calls `DeletePathSmart` per path, accumulating an `ExecResult`.

5. **`DeletePathSmart`** (`internal/cleaner/delete.go`) — for directories, batches immediate children to Recycle Bin in chunks of 64; then recycles the (now-empty) parent. Files/links go directly. Uses `internal/trash.MoveToRecycleBin`.

6. **`WriteStats` / `LoadStats`** (`internal/cleaner/stats.go`) — persists `ExecResult` as JSON to `~/.win-cleaner/stats/` (one file per run). The GUI's Stats dialog reads these.

### Key types

- `Registry` / `Item` — the static catalog of what to clean and where.
- `Plan` / `Group` — the runtime snapshot: resolved paths + measured sizes + selection state.
- `ExecResult` / `GroupResult` — JSON-serializable cleanup outcome stored in stats.

### Safety

- `isSafePath` requires a path to be *under* (not equal to) one of the four safe roots. Nothing outside those roots can be deleted.
- All deletions go through the Windows Recycle Bin (`SHFileOperationW` via `internal/trash/trash_windows.go`). Permanent deletion is never used.
- `RegistryConfig.SkipShaderCache` omits NVIDIA DXCache/GLCache when set.

### GUI

Built with Fyne v2. `RunGUI` drives a three-screen flow: Scan → Select → Delete (or Dry-run dialog). All UI mutations from goroutines use `a.Driver().DoFromGoroutine`. The Stats button opens `showStatsDialog` which reads persisted JSON runs.
