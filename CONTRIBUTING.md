# Contributing

Thanks for helping improve win-cleaner.

## Before opening a change

- Use an issue for substantial behavior or architecture changes.
- Keep platform-neutral behavior in `cleaner-core` and Windows APIs in `cleaner-platform`; `cleaner-app` is the only crate that may depend on egui.
- Do not weaken the safety model for convenience: the strictly-under safe-root guard (checked at scan time and again before every delete), Recycle-Bin-only deletion, dry-run default, opt-in empty-folder removal, and reparse-point/symlink exclusion.
- Changes to cleanup targets should describe the affected paths and why they are safe to remove.

## Verification

Run the portable checks before submitting a pull request:

```sh
cargo fmt --all -- --check
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

Changes to Windows-only behavior should also pass:

```sh
cargo xwin build --workspace --release --target x86_64-pc-windows-msvc
```

Describe any manual Windows testing in the pull request, especially for Recycle Bin behavior (locked files, huge directories, abort handling), known-folder resolution, and the GUI screens.
