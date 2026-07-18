# Windows Cleaner

[![CI](https://github.com/handsomefox/win-cleaner/actions/workflows/ci.yml/badge.svg)](https://github.com/handsomefox/win-cleaner/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

A Windows desktop app that scans known application and system cache locations, shows how much space each one holds, and cleans the ones you select.

## Features

- Curated catalog of 80+ cleanup targets: browsers, chat apps, game launchers, developer tools, GPU shader caches, and Windows system caches.
- Category → app → item selection tree with per-item size estimates, search, and sorting.
- Preview (dry-run) mode is the default; nothing is deleted until you explicitly clean.
- Opt-in detection of empty top-level folders under AppData and ProgramData.
- Run history with per-run details and 7/30-day totals.
- Diagnostics log you can attach to bug reports.

## Safety model

- Cache cleanup moves each selected top-level path to the **Recycle Bin** (`SHFileOperationW` with `FOF_ALLOWUNDO`) and never falls back to permanent deletion.
- Empty-folder cleanup uses a non-recursive directory removal that succeeds only while the folder is still empty, closing the scan-to-clean race without risking newly created files.
- Every path must be **strictly inside one of the known safe roots** (AppData Local/Roaming, ProgramData, the user profile, the Windows and Program Files trees). The guard is enforced when scanning **and re-checked immediately before every delete**; the roots themselves and paths that are, or are reached through, symlinks/reparse points are always rejected.
- Empty groups are never pre-selected, empty-folder removal is opt-in, and symlinks/reparse points are treated as content — never followed, never sized, never considered "empty".
- Cleaning writes a JSON record of exactly what was attempted to the run history.

## Diagnostics

The app writes logs to `%LOCALAPPDATA%\win-cleaner\logs\`. If something fails, attach the latest log file to your issue. Run history lives in `%LOCALAPPDATA%\win-cleaner\stats\`.

## Development

Portable logic (catalog, scanning, planning, execution strategy, statistics) builds and tests on Linux:

```sh
cargo fmt --all -- --check
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

Build the Windows 10/11 x86-64 application from Linux with `cargo-xwin`:

```sh
cargo xwin build --workspace --release --target x86_64-pc-windows-msvc
```

Create the portable executable, checksum, and ZIP under `dist/`:

```sh
bash scripts/package-windows.sh
```

The packaging script requires `cargo-xwin`, `zip`, and GNU `sha256sum`; it
verifies both the generated checksum and ZIP before returning success.

The GUI runs on Linux for development, but scanning and cleaning are Windows-only.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md). Please report security-sensitive issues according to [SECURITY.md](SECURITY.md).

## License

Licensed under the [MIT License](LICENSE). Bundled assets keep their own licenses: [Inter](https://rsms.me/inter/) (SIL OFL 1.1) and [Phosphor](https://phosphoricons.com/) icons (MIT).
