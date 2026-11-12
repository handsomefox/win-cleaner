# Repository guidelines

The [README](README.md) covers what the app does, its safety model, and the build and check commands. This page covers the rules for changing it.

## Where code goes

The workspace is layered so that everything portable stays testable on Linux:

- `cleaner-core` holds the model types, the safe-root guard in `safety.rs`, scanning, planning, execution, glob expansion, empty-folder handling, and statistics. No app-specific path knowledge lives here.
- `cleaner-catalog` holds the built-in catalog. `build_registry` derives every path from an injected `Roots`, so no username is hardcoded and tests can point everything at a temp directory.
- `cleaner-platform` wraps the Windows APIs: root discovery and the Recycle Bin in `recycle.rs`.
- `cleaner-app` is the egui binary. `worker.rs` runs blocking work off the UI thread, `viewmodel.rs` shapes state for `ui/`, and `strings.rs` holds the user-facing text.
- The shared crates come from [handsomefox/app-internals](https://github.com/handsomefox/app-internals), pinned to a tag: `app-update` updates the app from its GitHub releases, `app-update-ui` draws the update dialog's content, `app-diagnostics` writes the log files, and `app-folders` resolves the four user roots and opens folders. That repository's `AGENTS.md` covers changing them. `cleaner-app/src/ui/update.rs` holds this app's chip, its modal, and the `Look` that themes the dialog.

One catalog entry is a `cleaner_core::Item` in code and a "cleanup target" in the UI and the README. Keep those two names attached to those two contexts.

## Do not weaken the safety model

This app deletes files. Five rules hold it up, and none of them is negotiable:

- Every path must sit strictly inside a known safe root. The guard runs at scan time and again immediately before each delete. The roots themselves are always rejected.
- Deletion goes to the Recycle Bin through `IFileOperation` with `FOFX_RECYCLEONDELETE | FOFX_ADDUNDORECORD`. The progress sink in `recycle.rs` aborts any delete that would not go to the Recycle Bin. There is no permanent-deletion fallback. A failed move leaves the path alone.
- Symlinks and reparse points count as content. Never follow one, never add its target to a size estimate, never treat a folder holding one as empty.
- Preview is the default.
- Empty-folder removal stays opt-in, and uses a non-recursive removal that succeeds only while the folder is still empty. Only the user's own click checks an empty-folder row: ticking it, or picking a preset that holds it. The elevated copy carries that click across the restart. A scan, the startup preset, and the remembered selection never check one.

The user's rules in `cleaner-core/src/rules.rs` hold up three more:

- Cleanup plans each target again against the rules as they are when it runs, not as they were at the scan. An exclusion added after the scan still protects its path.
- A folder that holds an exclusion is never moved whole. Cleanup moves the children around the exclusion instead.
- An old program version is moved whole or not at all. Age and size limits never apply to it, and an exclusion inside it leaves all of it in place.

An elevated copy cleans only when it has confirmed, by SID, that it runs as the signed-in account. A copy that runs as another account, or that cannot tell, cleans nothing, because the user folders it finds may belong to someone else. An old program version that cannot be moved whole stays where it is: cleanup never moves its files one by one.

## Tests

Unit tests live beside their implementations in `#[cfg(test)]` modules. Use `tempfile` for filesystem scenarios.

Cover the rejection path as well as the success path, especially for the safe-root guard, glob expansion, symlink and reparse-point handling, and empty-folder removal. No coverage percentage is required, but a safety regression should have a test that catches it.

CI cannot reach the Recycle Bin. Exercise locked files, very large directories, and aborting a run by hand on Windows, along with known-folder resolution. The same goes for an app update: run a debug build with `APP_UPDATE_PRETEND_VERSION` set to an older version.

## CI and releases

`.github/workflows/ci.yml` and `release.yml` only call the shared workflows in [handsomefox/app-workflows](https://github.com/handsomefox/app-workflows), pinned by commit. The checks, the release steps, and the CI tool pins live there, and its `AGENTS.md` covers changing them. Dependabot moves the pin when that repository releases.

CI runs actionlint and `zizmor --persona pedantic` on every push. Run both before you push a workflow change.
