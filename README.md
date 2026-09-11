# Windows Cleaner

[![CI](https://github.com/handsomefox/win-cleaner/actions/workflows/ci.yml/badge.svg)](https://github.com/handsomefox/win-cleaner/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

A Windows desktop app that scans known cache locations, shows how much space each one holds, and moves the ones you pick to the Recycle Bin.

## Features

- A built-in catalog of 82 cleanup targets covering browsers, chat apps, game launchers, developer tools, GPU shader caches, and Windows system caches. To regenerate that count, run `cargo test -p cleaner-catalog full_registry_shape`, which asserts it.
- A selection tree of category, then app, then target, with a size estimate on each target, plus search and sorting.
- Preview mode by default. Nothing is deleted until you start a clean.
- Optional detection of empty top-level folders under AppData and ProgramData.
- Run history with per-run detail and totals for the last 7 and last 30 days.
- A diagnostics log you can attach to a bug report.

## Safety model

Cache cleanup moves each selected top-level path to the Recycle Bin, using `SHFileOperationW` with `FOF_ALLOWUNDO`. There is no fallback to permanent deletion: if the move fails, the path stays.

Empty-folder cleanup uses a non-recursive directory removal that succeeds only while the folder is still empty. Something can be written into a folder between the scan and the clean, and this closes that window without risking the new file.

Every path must sit strictly inside one of the known safe roots: AppData Local and Roaming, ProgramData, your user profile, and the Windows and Program Files trees. The app enforces that check while scanning, and checks it again immediately before each delete. It always rejects the roots themselves. It also rejects any path that is a symlink or reparse point, or that is reached through one.

Symlinks and reparse points count as content everywhere else too. The app never follows one, never adds its target to a size estimate, and never treats a folder containing one as empty. Groups that turn out to be empty are never pre-selected, and empty-folder removal stays off until you turn it on.

Each clean writes a JSON record of what it attempted to the run history.

## Diagnostics

The app writes logs to `%LOCALAPPDATA%\win-cleaner\logs\` and run history to `%LOCALAPPDATA%\win-cleaner\stats\`. If something fails, attach the newest log file to your issue.

## Development

The portable logic (catalog, scanning, planning, execution strategy, and statistics) builds and tests on any OS, including Linux:

```sh
cargo fmt --all -- --check
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

CI runs those three commands on Ubuntu and the last two on Windows. It also runs `cargo audit` and `cargo machete`. Only the release workflow builds release binaries.

To cross-build the Windows 10/11 x86-64 app from Linux, use `cargo-xwin`:

```sh
cargo xwin build --workspace --release --target x86_64-pc-windows-msvc
```

To produce the portable executable, its SHA-256 checksum, and a ZIP under `dist/`:

```sh
bash scripts/package-windows.sh
```

The packaging script needs `cargo-xwin`, `zip`, and GNU `sha256sum`. It verifies the checksum file and the ZIP before it reports success.

The GUI runs on Linux for development work, but scanning and cleaning call Windows APIs and only work there.

## License

Licensed under the [MIT License](LICENSE). Bundled assets keep their own licenses: the [Inter](https://rsms.me/inter/) typeface under SIL OFL 1.1, and the [Phosphor](https://phosphoricons.com/) icons under MIT.
