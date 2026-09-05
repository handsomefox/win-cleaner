# Repository guidelines

The [README](README.md) covers what the app does, its safety model, and the build and check commands. This page covers the rules for changing it.

## Where code goes

The workspace is layered so that everything portable stays testable on Linux:

- `cleaner-core` holds the model types, the safe-root guard in `safety.rs`, scanning, planning, execution, glob expansion, empty-folder handling, and statistics. No app-specific path knowledge lives here.
- `cleaner-catalog` holds the built-in catalog. `build_registry` derives every path from an injected `Roots`, so no username is hardcoded and tests can point everything at a temp directory.
- `cleaner-platform` wraps the Windows APIs: known-folder resolution, root discovery, and the Recycle Bin in `recycle.rs`.
- `cleaner-app` is the egui binary. `worker.rs` runs blocking work off the UI thread, `viewmodel.rs` shapes state for `ui/`, and `strings.rs` holds the user-facing text.

One catalog entry is a `cleaner_core::Item` in code and a "cleanup target" in the UI and the README. Keep those two names attached to those two contexts.

## Do not weaken the safety model

This app deletes files. Five rules hold it up, and none of them is negotiable:

- Every path must sit strictly inside a known safe root. The guard runs at scan time and again immediately before each delete. The roots themselves are always rejected.
- Deletion goes to the Recycle Bin through `SHFileOperationW` with `FOF_ALLOWUNDO`. There is no permanent-deletion fallback. A failed move leaves the path alone.
- Symlinks and reparse points count as content. Never follow one, never add its target to a size estimate, never treat a folder holding one as empty.
- Preview is the default.
- Empty-folder removal stays opt-in, and uses a non-recursive removal that succeeds only while the folder is still empty.

## Tests

Unit tests live beside their implementations in `#[cfg(test)]` modules. Use `tempfile` for filesystem scenarios.

Cover the rejection path as well as the success path, especially for the safe-root guard, glob expansion, symlink and reparse-point handling, and empty-folder removal. No coverage percentage is required, but a safety regression should have a test that catches it.

CI cannot reach the Recycle Bin. Exercise locked files, very large directories, and aborting a run by hand on Windows, along with known-folder resolution.
