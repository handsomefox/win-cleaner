//! Plan execution. Cache paths are moved to the Recycle Bin via a [`Recycler`]
//! implementation. Empty-directory cleanup uses an atomic, non-recursive
//! removal so a directory that gains content after scanning is never deleted.
//! The safety guard is re-checked immediately before every operation, with the
//! folders above the path held in place until the operation ends.

use std::fs;
use std::path::{MAIN_SEPARATOR, Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::SystemTime;

use crate::empty_folders::EMPTY_FOLDERS_APP;
use crate::error::RecycleError;
use crate::plan::{Group, Options, Plan, ProgressUpdate, Target};
use crate::rules::{Rules, Walk, plan_paths};
use crate::safety::{is_link, is_reparse_point, is_safe_path, normalized_key};
use crate::scan::dir_size;
use crate::stats::{ExecResult, GroupResult, PathError};
use crate::versions::superseded;

/// Moves paths to the Recycle Bin. The single production implementation wraps
/// `IFileOperation` in `cleaner-platform`; tests use mocks.
pub trait Recycler {
    /// Moves one or more absolute paths to the Recycle Bin in a single call.
    ///
    /// # Errors
    ///
    /// Returns an error when the shell refuses or aborts the operation.
    fn recycle(&self, paths: &[&Path]) -> Result<(), RecycleError>;
}

/// What a cleanup runs under, besides the plan: the safe roots, the user's
/// rules as they are now, the flag that stops it, and how to tell whether a
/// program is running.
pub struct Run<'a> {
    pub options: Options,
    pub guard_roots: &'a [&'a Path],
    pub rules: &'a Rules,
    /// Set from another thread to stop before the next path. The path being
    /// moved finishes; nothing after it starts.
    pub stop: &'a AtomicBool,
    /// Whether a process with this executable name is running.
    pub is_running: &'a dyn Fn(&str) -> bool,
}

/// Executes the selected groups of `plan`, re-guarding every path against
/// the safe roots. As a defense in depth (the GUI never calls this in preview
/// mode) a dry run performs no deletions and reports no groups.
///
/// Each target is planned again here, against the rules as they are now: a
/// path excluded since the scan stays, and a folder that has gained an
/// exclusion is split around it instead of being moved whole.
pub fn execute_with_result(
    plan: &Plan,
    run: &Run<'_>,
    recycler: &dyn Recycler,
    mut progress: impl FnMut(ProgressUpdate),
) -> ExecResult {
    let mut result = ExecResult::begin(plan, run.options);
    if plan.selected == 0 || run.options.dry_run {
        result.finish();
        return result;
    }

    let total = plan.groups.iter().filter(|group| group.on).count();
    for (index, group) in plan.groups.iter().filter(|group| group.on).enumerate() {
        if run.stop.load(Ordering::Relaxed) {
            result.stopped = true;
            break;
        }
        progress(ProgressUpdate {
            current: index + 1,
            total,
            message: format!("{} - {}", group.app, group.label),
        });

        let mut group_result = GroupResult {
            app: group.app.clone(),
            label: group.label.clone(),
            errors: Vec::new(),
            bytes: group.bytes,
            freed: Some(0),
            skipped: None,
            paths_attempted: 0,
            paths_failed: 0,
            stopped: false,
            not_recycled: 0,
            recycle_bin_off: false,
        };
        let mut halted = false;
        if run.rules.skip_running_apps
            && let Some(name) = group.processes.iter().find(|name| (run.is_running)(name))
        {
            group_result.skipped = Some(format!("{name} is running"));
        } else {
            match clean_group(group, run, recycler, &mut group_result) {
                Ended::Finished => {}
                Ended::Stopped => group_result.stopped = true,
                Ended::NotRecycled => {
                    group_result.stopped = true;
                    halted = true;
                }
            }
        }
        if halted || run.stop.load(Ordering::Relaxed) {
            result.stopped = true;
        }
        result.error_count += group_result.errors.len();
        result.freed_bytes = result
            .freed_bytes
            .map(|freed| freed + group_result.freed.unwrap_or(0));
        result.groups.push(group_result);
        // The shell deleted something for good. It would most likely do the
        // same to everything after it, such as when the Recycle Bin is off
        // for the drive, so nothing else is tried.
        if halted {
            break;
        }
    }
    result.finish();
    result
}

/// How cleaning one target ended.
enum Ended {
    /// Every path in it was tried.
    Finished,
    /// Stop cut it short.
    Stopped,
    /// The shell deleted something in it for good instead of moving it to the
    /// Recycle Bin, or would have, so the run ends here.
    NotRecycled,
}

/// Cleans one target.
fn clean_group(
    group: &Group,
    run: &Run<'_>,
    recycler: &dyn Recycler,
    out: &mut GroupResult,
) -> Ended {
    let now = SystemTime::now();
    for target in &group.targets {
        if run.stop.load(Ordering::Relaxed) {
            return Ended::Stopped;
        }
        let path = target.path.as_path();
        if !is_safe_path(path, run.guard_roots) {
            out.errors.push(PathError {
                path: path.display().to_string(),
                error: "unsafe path (guard)".to_owned(),
            });
            continue;
        }
        if fs::symlink_metadata(path).is_err() || run.rules.is_excluded(path) {
            continue;
        }
        if group.app == EMPTY_FOLDERS_APP {
            out.paths_attempted += 1;
            if let Err(error) = remove_empty_directory(path, run.guard_roots) {
                out.paths_failed += 1;
                out.errors.push(PathError {
                    path: path.display().to_string(),
                    error: error.to_string(),
                });
            }
            continue;
        }
        // A program version is moved whole or not at all: half of one is
        // worse than either. Age and size limits do not apply to it.
        if target.version_rule.is_some() {
            if run.rules.has_exclusion_inside(path) || !still_superseded(target) {
                continue;
            }
            if clean_path(path, target.bytes, run, recycler, out, Salvage::Never) {
                return Ended::NotRecycled;
            }
            continue;
        }
        let whole = !target.keep_folder
            && !run.rules.filters_files()
            && !run.rules.has_exclusion_inside(path);
        if whole {
            if clean_path(path, target.bytes, run, recycler, out, Salvage::Children) {
                return Ended::NotRecycled;
            }
            continue;
        }
        if target.keep_folder && !is_real_folder(path) {
            out.paths_attempted += 1;
            out.paths_failed += 1;
            out.errors.push(PathError {
                path: path.display().to_string(),
                error: "not a folder whose contents can be recycled".to_owned(),
            });
            continue;
        }
        let mut planned = Vec::new();
        let mut walk = Walk::measured().stopping_on(run.stop);
        plan_paths(
            path,
            target.keep_folder,
            run.rules,
            now,
            &mut walk,
            &mut |child, size| {
                planned.push((child.to_path_buf(), size));
                !run.stop.load(Ordering::Relaxed)
            },
        );
        if run.stop.load(Ordering::Relaxed) {
            return Ended::Stopped;
        }
        // A folder that could not be listed, such as one that needs
        // administrator rights, is a failure, not "nothing to clean".
        if !walk.errors.is_empty() {
            out.paths_attempted += 1;
            out.paths_failed += 1;
            out.errors.push(PathError {
                path: path.display().to_string(),
                error: walk.errors.join("; "),
            });
        }
        for (child, size) in planned {
            if run.stop.load(Ordering::Relaxed) {
                return Ended::Stopped;
            }
            if run.rules.filters_files() && !still_passes(&child, run.rules, now) {
                continue;
            }
            if clean_path(&child, size, run, recycler, out, Salvage::Children) {
                return Ended::NotRecycled;
            }
        }
    }
    Ended::Finished
}

/// Whether a file planned under an age or size limit may still go. Other
/// programs keep writing while a cleanup runs: a file rewritten since it was
/// planned is new again, and one replaced by a folder or a link would move
/// everything in it.
fn still_passes(path: &Path, rules: &Rules, now: SystemTime) -> bool {
    fs::symlink_metadata(path).is_ok_and(|metadata| {
        metadata.is_file()
            && !is_link(&metadata)
            && !is_reparse_point(&metadata)
            && !rules.is_excluded(path)
            && rules.file_passes(&metadata, now)
    })
}

/// What to do when a folder cannot be moved whole.
#[derive(Clone, Copy)]
enum Salvage {
    /// Move what can be moved inside it, child by child.
    Children,
    /// Leave all of it. An old program version is moved whole or not at all.
    Never,
}

/// Moves one planned path, records the outcome, and counts what left.
///
/// Returns `true` when the run must end: the shell deleted some of it for good
/// instead of moving it to the Recycle Bin, or refused to move it because the
/// Recycle Bin is off or too small for its drive. Each path deleted for good
/// is recorded as an error, and none of `planned_bytes` counts as freed, since
/// nothing of it can be restored.
fn clean_path(
    path: &Path,
    planned_bytes: u64,
    run: &Run<'_>,
    recycler: &dyn Recycler,
    out: &mut GroupResult,
    salvage: Salvage,
) -> bool {
    out.paths_attempted += 1;
    let mut destroyed = Vec::new();
    let moved = match salvage {
        Salvage::Children => {
            delete_path_smart(path, run.guard_roots, recycler, run.stop, &mut destroyed)
        }
        Salvage::Never => move_whole(path, run.guard_roots, recycler, &mut destroyed),
    };
    if matches!(moved, Err(RecycleError::NotRecycled)) && destroyed.is_empty() {
        destroyed.push(path.to_path_buf());
    }
    if let Err(error) = &moved {
        out.paths_failed += 1;
        out.recycle_bin_off |= matches!(error, RecycleError::RecycleBinOff);
        // A permanent delete is recorded below, once per path it took.
        if !matches!(error, RecycleError::NotRecycled) {
            out.errors.push(PathError {
                path: path.display().to_string(),
                error: error.to_string(),
            });
        }
    }
    if !destroyed.is_empty() {
        out.not_recycled += destroyed.len();
        out.errors.extend(destroyed.iter().map(|lost| PathError {
            path: lost.display().to_string(),
            error: RecycleError::NotRecycled.to_string(),
        }));
        return true;
    }
    // Everything after it would most likely be refused the same way.
    if matches!(moved, Err(RecycleError::RecycleBinOff)) {
        return true;
    }
    let remaining = if path_missing(path) {
        0
    } else {
        dir_size(path).unwrap_or(planned_bytes)
    };
    out.freed = out
        .freed
        .map(|freed| freed + planned_bytes.saturating_sub(remaining));
    false
}

fn is_real_folder(path: &Path) -> bool {
    fs::symlink_metadata(path)
        .is_ok_and(|metadata| metadata.file_type().is_dir() && !is_link(&metadata))
}

/// Whether a program version found at scan time is still superseded. An
/// update in between can make it the installed one, so the rule is asked
/// again; a path no version rule found always proceeds.
fn still_superseded(target: &Target) -> bool {
    let Some(rule) = &target.version_rule else {
        return true;
    };
    let key = normalized_key(&target.path);
    superseded(rule)
        .iter()
        .any(|path| normalized_key(path) == key)
}

/// First attempts to move exactly `path` to the Recycle Bin in one operation.
///
/// When that fails for a real directory, its immediate children are retried
/// independently. The fallback never recurses, never submits a symlink or
/// reparse point, and re-runs the execution-time safety guard for every child.
/// Successful child cleanup is retained even if other children remain locked.
///
/// Every path the shell deletes for good instead of moving to the Recycle Bin
/// goes to `destroyed`, and nothing more is tried after the first.
///
/// # Errors
///
/// Returns [`RecycleError::NotRecycled`] when anything went to `destroyed`,
/// an error when `path` cannot be inspected, and an aggregate error when
/// salvage is incomplete and the directory still exists.
pub fn delete_path_smart(
    path: &Path,
    guard_roots: &[&Path],
    recycler: &dyn Recycler,
    stop: &AtomicBool,
    destroyed: &mut Vec<PathBuf>,
) -> Result<(), RecycleError> {
    if !is_safe_path(path, guard_roots) {
        return if path_missing(path) {
            Ok(())
        } else {
            Err(RecycleError::UnsafePath(path.to_path_buf()))
        };
    }
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        // Without its metadata, nothing says what the path is now.
        Err(source) => {
            return Err(RecycleError::Uninspectable {
                path: path.to_path_buf(),
                source,
            });
        }
    };
    let is_real_directory = metadata.file_type().is_dir() && !is_link(&metadata);
    // A permanent delete leaves the path missing too, so it is matched first.
    let initial_error = match guarded_recycle(path, guard_roots, recycler) {
        Ok(()) => return Ok(()),
        Err(RecycleError::NotRecycled) => {
            destroyed.push(path.to_path_buf());
            return Err(RecycleError::NotRecycled);
        }
        // Its children would go the same way, so none is tried.
        Err(RecycleError::RecycleBinOff) => return Err(RecycleError::RecycleBinOff),
        Err(_) if path_missing(path) => return Ok(()),
        Err(error) => error,
    };
    if !is_real_directory {
        return Err(initial_error);
    }

    let mut failures = vec![format!("{}: {initial_error}", path.display())];
    let finished = recycle_children(path, guard_roots, recycler, stop, &mut failures, destroyed);
    if !destroyed.is_empty() {
        return Err(RecycleError::NotRecycled);
    }
    if !finished {
        return if path_missing(path) {
            Ok(())
        } else {
            Err(RecycleError::Multiple(failures.join("; ")))
        };
    }

    match guarded_recycle(path, guard_roots, recycler) {
        Ok(()) => {}
        Err(RecycleError::NotRecycled) => {
            destroyed.push(path.to_path_buf());
            return Err(RecycleError::NotRecycled);
        }
        Err(_) if path_missing(path) => return Ok(()),
        Err(RecycleError::UnsafePath(_)) => {
            failures.push(format!("parent safety guard failed: {}", path.display()));
        }
        Err(error) => failures.push(format!("{}: {error}", path.display())),
    }
    if path_missing(path) {
        Ok(())
    } else {
        Err(RecycleError::Multiple(failures.join("; ")))
    }
}

/// Moves `path` in one Recycle Bin operation, after the safety guard, and
/// leaves it untouched when that fails. A permanent delete goes to
/// `destroyed`.
fn move_whole(
    path: &Path,
    guard_roots: &[&Path],
    recycler: &dyn Recycler,
    destroyed: &mut Vec<PathBuf>,
) -> Result<(), RecycleError> {
    match guarded_recycle(path, guard_roots, recycler) {
        Err(RecycleError::NotRecycled) => {
            destroyed.push(path.to_path_buf());
            Err(RecycleError::NotRecycled)
        }
        Err(_) if path_missing(path) => Ok(()),
        moved => moved,
    }
}

/// Moves `path` to the Recycle Bin with the folders above it held in place.
///
/// The shell finds `path` by name. Between a guard check and that lookup,
/// another program could swap a folder above `path` for a junction, and the
/// move would then take something outside the safe roots. So the folders from
/// the safe root down to the parent of `path` are opened first and held, then
/// the guard runs, then the shell moves `path`. See [`hold_parents`].
fn guarded_recycle(
    path: &Path,
    guard_roots: &[&Path],
    recycler: &dyn Recycler,
) -> Result<(), RecycleError> {
    let _held = hold_parents(path, guard_roots).map_err(|source| RecycleError::ParentNotHeld {
        path: path.to_path_buf(),
        source,
    })?;
    if !is_safe_path(path, guard_roots) {
        return Err(RecycleError::UnsafePath(path.to_path_buf()));
    }
    recycler.recycle(&[path])
}

/// Opens every folder from the outermost safe root holding `path` down to the
/// parent of `path`, top down, and returns the open handles.
///
/// On Windows each folder is opened as itself, never through a link, and
/// without sharing delete access. While the handles are open no other program
/// can rename, delete, or replace those folders, and none of them can become a
/// junction, since a folder that holds anything cannot. Opening top down means
/// each folder is reached through folders already held. A folder below the
/// root that is a link or reparse point fails the hold, as it fails the guard.
///
/// Elsewhere nothing is held, and the guard alone applies.
///
/// # Errors
///
/// Returns the error from opening a folder, or one for a folder that is a link
/// or not a folder. A path under no safe root holds nothing and returns no
/// error: the guard rejects it next.
fn hold_parents(path: &Path, guard_roots: &[&Path]) -> std::io::Result<Vec<Held>> {
    let key = normalized_key(path);
    let Some(root) = guard_roots
        .iter()
        .map(|root| normalized_key(root))
        .filter(|root| key.starts_with(&format!("{root}{MAIN_SEPARATOR}")))
        .min_by_key(String::len)
    else {
        return Ok(Vec::new());
    };
    let mut folders: Vec<&Path> = Vec::new();
    for folder in path.ancestors().skip(1) {
        folders.push(folder);
        if normalized_key(folder) == root {
            break;
        }
    }
    folders.reverse();
    let mut held = Vec::with_capacity(folders.len());
    for (depth, folder) in folders.into_iter().enumerate() {
        // The root itself is trusted as it is, as the guard trusts it.
        held.push(hold_folder(folder, depth > 0)?);
    }
    Ok(held)
}

/// An open folder, held in place until it drops.
#[cfg(windows)]
type Held = fs::File;
#[cfg(not(windows))]
type Held = ();

#[cfg(windows)]
fn hold_folder(folder: &Path, refuse_link: bool) -> std::io::Result<Held> {
    use std::os::windows::fs::OpenOptionsExt;
    const FILE_LIST_DIRECTORY: u32 = 0x0001;
    const FILE_READ_ATTRIBUTES: u32 = 0x0080;
    const SYNCHRONIZE: u32 = 0x0010_0000;
    const FILE_SHARE_READ: u32 = 0x0001;
    const FILE_SHARE_WRITE: u32 = 0x0002;
    const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
    const FILE_FLAG_BACKUP_SEMANTICS: u32 = 0x0200_0000;
    // A handle with only attribute access skips the sharing check, so it
    // would not stop a rename. Listing access takes part in it.
    let handle = fs::OpenOptions::new()
        .access_mode(FILE_LIST_DIRECTORY | FILE_READ_ATTRIBUTES | SYNCHRONIZE)
        .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE)
        .custom_flags(FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT)
        .open(folder)?;
    let metadata = handle.metadata()?;
    if !metadata.is_dir() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotADirectory,
            format!("{} is not a folder", folder.display()),
        ));
    }
    if refuse_link && is_link(&metadata) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("{} is a link or reparse point", folder.display()),
        ));
    }
    Ok(handle)
}

/// Unix has no share modes, so nothing is held and the guard alone applies.
#[cfg(not(windows))]
#[expect(
    clippy::unnecessary_wraps,
    reason = "matches the Windows version, which can fail"
)]
fn hold_folder(_folder: &Path, _refuse_link: bool) -> std::io::Result<Held> {
    Ok(())
}

/// Recycles each immediate child of `path` on its own, re-running the safety
/// guard for each and never submitting a link. Problems go to `failures`, and
/// a child the shell deleted for good goes to `destroyed`. Returns `false`
/// when the folder could not be listed at all, when `stop` was set before
/// every child was tried, or when a child was deleted for good.
fn recycle_children(
    path: &Path,
    guard_roots: &[&Path],
    recycler: &dyn Recycler,
    stop: &AtomicBool,
    failures: &mut Vec<String>,
    destroyed: &mut Vec<PathBuf>,
) -> bool {
    let entries = match fs::read_dir(path) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return false,
        Err(error) => {
            failures.push(format!("could not enumerate {}: {error}", path.display()));
            return false;
        }
    };
    for entry in entries {
        if stop.load(Ordering::Relaxed) {
            failures.push(format!(
                "stopped before everything in {} was moved",
                path.display()
            ));
            return false;
        }
        let entry = match entry {
            Ok(entry) => entry,
            Err(error) => {
                failures.push(format!("could not enumerate a child: {error}"));
                continue;
            }
        };
        let child = entry.path();
        // Windows drops a trailing dot or space from a name it looks up, so
        // the shell would move some other item than this one.
        if crate::rules::trimmed_by_windows(&entry.file_name()) {
            failures.push(format!(
                "{}: its name ends in a dot or a space, so it was left in place",
                child.display()
            ));
            continue;
        }
        let child_metadata = match fs::symlink_metadata(&child) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => {
                failures.push(format!("could not inspect {}: {error}", child.display()));
                continue;
            }
        };
        if is_link(&child_metadata) || !is_safe_path(&child, guard_roots) {
            failures.push(format!("unsafe child retained: {}", child.display()));
            continue;
        }
        match guarded_recycle(&child, guard_roots, recycler) {
            Err(RecycleError::NotRecycled) => {
                destroyed.push(child);
                return false;
            }
            Err(error) if !path_missing(&child) => {
                failures.push(format!("{}: {error}", child.display()));
            }
            _ => {}
        }
    }
    // Stop pressed while the last child moved still means no parent retry.
    if stop.load(Ordering::Relaxed) {
        failures.push(format!(
            "stopped before everything in {} was moved",
            path.display()
        ));
        return false;
    }
    true
}

fn path_missing(path: &Path) -> bool {
    matches!(
        fs::symlink_metadata(path),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound
    )
}

/// Removes an empty directory without recursion. `remove_dir` performs the
/// emptiness check and removal as one filesystem operation, so a directory
/// that gains a file after the scan fails closed instead of deleting it. The
/// folders above it are held in place and the guard runs again first, as for
/// a move to the Recycle Bin.
fn remove_empty_directory(path: &Path, guard_roots: &[&Path]) -> std::io::Result<()> {
    let held = match hold_parents(path, guard_roots) {
        Ok(held) => held,
        Err(_) if path_missing(path) => return Ok(()),
        Err(err) => return Err(err),
    };
    if !is_safe_path(path, guard_roots) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "unsafe path (guard)",
        ));
    }
    let removed = fs::remove_dir(path);
    drop(held);
    match removed {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(err),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan::Group;
    use std::cell::RefCell;
    use std::collections::HashSet;
    use std::fs::{File, create_dir_all};
    use std::path::PathBuf;

    /// [`super::delete_path_smart`] for a test in which the shell deletes
    /// nothing for good.
    fn delete_path_smart(
        path: &Path,
        guard_roots: &[&Path],
        recycler: &dyn Recycler,
        stop: &AtomicBool,
    ) -> Result<(), RecycleError> {
        let mut destroyed = Vec::new();
        let result = super::delete_path_smart(path, guard_roots, recycler, stop, &mut destroyed);
        assert_eq!(destroyed, Vec::<PathBuf>::new());
        result
    }

    /// Records every batch; successful calls remove the mocked paths.
    struct MockRecycler {
        calls: RefCell<Vec<Vec<PathBuf>>>,
    }

    impl MockRecycler {
        fn new() -> Self {
            Self {
                calls: RefCell::new(Vec::new()),
            }
        }

        fn call_sizes(&self) -> Vec<usize> {
            self.calls.borrow().iter().map(Vec::len).collect()
        }
    }

    impl Recycler for MockRecycler {
        fn recycle(&self, paths: &[&Path]) -> Result<(), RecycleError> {
            self.calls
                .borrow_mut()
                .push(paths.iter().map(|p| p.to_path_buf()).collect());
            for path in paths {
                if path.is_dir() {
                    fs::remove_dir_all(path).ok();
                } else {
                    fs::remove_file(path).ok();
                }
            }
            Ok(())
        }
    }

    struct FailingRecycler {
        calls: RefCell<Vec<PathBuf>>,
        fail_once: RefCell<HashSet<PathBuf>>,
        fail_always: HashSet<PathBuf>,
        disappear_on_failure: Option<PathBuf>,
    }

    impl FailingRecycler {
        fn new() -> Self {
            Self {
                calls: RefCell::new(Vec::new()),
                fail_once: RefCell::new(HashSet::new()),
                fail_always: HashSet::new(),
                disappear_on_failure: None,
            }
        }

        fn calls(&self) -> Vec<PathBuf> {
            self.calls.borrow().clone()
        }
    }

    impl Recycler for FailingRecycler {
        fn recycle(&self, paths: &[&Path]) -> Result<(), RecycleError> {
            assert_eq!(paths.len(), 1);
            let path = paths[0].to_path_buf();
            self.calls.borrow_mut().push(path.clone());
            let fails_once = self.fail_once.borrow_mut().remove(&path);
            if fails_once || self.fail_always.contains(&path) {
                if self.disappear_on_failure.as_ref() == Some(&path) {
                    fs::remove_dir_all(&path).ok();
                    fs::remove_file(&path).ok();
                }
                return Err(RecycleError::ShellOperation {
                    code: 5,
                    message: String::new(),
                });
            }
            if path.is_dir() {
                fs::remove_dir_all(path).ok();
            } else {
                fs::remove_file(path).ok();
            }
            Ok(())
        }
    }

    fn make_children(dir: &Path, count: usize) {
        create_dir_all(dir).unwrap();
        for i in 0..count {
            File::create(dir.join(format!("f{i:04}"))).unwrap();
        }
    }

    #[test]
    fn file_is_recycled_directly() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("a.txt");
        File::create(&file).unwrap();
        let mock = MockRecycler::new();
        delete_path_smart(&file, &[dir.path()], &mock, &AtomicBool::new(false)).unwrap();
        assert_eq!(mock.call_sizes(), vec![1]);
        assert!(!file.exists());
    }

    #[test]
    fn missing_path_is_ok_without_calls() {
        let dir = tempfile::tempdir().unwrap();
        let mock = MockRecycler::new();
        delete_path_smart(
            &dir.path().join("missing"),
            &[dir.path()],
            &mock,
            &AtomicBool::new(false),
        )
        .unwrap();
        assert_eq!(mock.call_sizes(), Vec::<usize>::new());
    }

    #[test]
    fn directory_is_recycled_as_one_top_level_path() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("cache");
        make_children(&target, 100);
        let mock = MockRecycler::new();
        delete_path_smart(&target, &[dir.path()], &mock, &AtomicBool::new(false)).unwrap();
        assert_eq!(mock.call_sizes(), vec![1]);
        assert!(!target.exists());
    }

    #[test]
    fn failed_directory_recycle_salvages_immediate_children() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("cache");
        make_children(&target, 3);
        let mut recycler = FailingRecycler::new();
        recycler.fail_once.get_mut().insert(target.clone());

        delete_path_smart(&target, &[dir.path()], &recycler, &AtomicBool::new(false)).unwrap();

        let calls = recycler.calls();
        assert_eq!(calls.first(), Some(&target));
        assert_eq!(calls.last(), Some(&target));
        assert_eq!(calls.len(), 5);
        assert!(!target.exists());
    }

    #[test]
    fn locked_child_does_not_prevent_other_children_from_being_recycled() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("cache");
        make_children(&target, 3);
        let locked = target.join("f0001");
        let removable = [target.join("f0000"), target.join("f0002")];
        let mut recycler = FailingRecycler::new();
        recycler.fail_always.insert(target.clone());
        recycler.fail_always.insert(locked.clone());

        let error = delete_path_smart(&target, &[dir.path()], &recycler, &AtomicBool::new(false))
            .unwrap_err();

        assert!(locked.exists());
        assert!(removable.iter().all(|path| !path.exists()));
        let message = error.to_string();
        assert!(message.contains("f0001"));
        assert!(message.contains("Recycle Bin operation failed"));
    }

    #[cfg(unix)]
    #[test]
    fn fallback_never_submits_or_traverses_symlink_children() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("cache");
        let outside = dir.path().join("outside");
        make_children(&target, 1);
        make_children(&outside, 1);
        let link = target.join("linked");
        std::os::unix::fs::symlink(&outside, &link).unwrap();
        let mut recycler = FailingRecycler::new();
        recycler.fail_always.insert(target.clone());

        let error = delete_path_smart(&target, &[dir.path()], &recycler, &AtomicBool::new(false))
            .unwrap_err();

        assert!(!target.join("f0000").exists());
        assert!(outside.join("f0000").exists());
        assert!(fs::symlink_metadata(&link).is_ok());
        assert!(!recycler.calls().contains(&link));
        assert!(error.to_string().contains("unsafe child retained"));
    }

    #[test]
    fn disappearance_after_failed_shell_operation_is_success() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("cache");
        make_children(&target, 2);
        let mut recycler = FailingRecycler::new();
        recycler.fail_once.get_mut().insert(target.clone());
        recycler.disappear_on_failure = Some(target.clone());

        delete_path_smart(&target, &[dir.path()], &recycler, &AtomicBool::new(false)).unwrap();
        assert_eq!(recycler.calls(), [target]);
    }

    #[test]
    fn stop_ends_the_salvage_of_a_locked_folder() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("cache");
        make_children(&target, 3);
        let mut recycler = FailingRecycler::new();
        recycler.fail_always.insert(target.clone());

        let error = delete_path_smart(&target, &[dir.path()], &recycler, &AtomicBool::new(true))
            .unwrap_err();

        assert_eq!(
            recycler.calls(),
            std::slice::from_ref(&target),
            "no child is tried"
        );
        assert_eq!(fs::read_dir(&target).unwrap().count(), 3);
        assert!(error.to_string().contains("stopped before everything"));
    }

    /// Refuses `parent` and moves anything else, setting `stop` as it does,
    /// the way a Stop click lands while the last child of a locked folder
    /// moves.
    struct StopOnChild<'a> {
        parent: PathBuf,
        stop: &'a AtomicBool,
        calls: RefCell<Vec<PathBuf>>,
    }

    impl Recycler for StopOnChild<'_> {
        fn recycle(&self, paths: &[&Path]) -> Result<(), RecycleError> {
            self.calls.borrow_mut().push(paths[0].to_path_buf());
            if paths[0] == self.parent {
                return Err(RecycleError::ShellOperation {
                    code: 32,
                    message: String::new(),
                });
            }
            fs::remove_file(paths[0]).ok();
            self.stop.store(true, Ordering::Relaxed);
            Ok(())
        }
    }

    #[test]
    fn stop_during_the_last_salvaged_child_leaves_the_parent() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("cache");
        make_children(&target, 1);
        let stop = AtomicBool::new(false);
        let recycler = StopOnChild {
            parent: target.clone(),
            stop: &stop,
            calls: RefCell::new(Vec::new()),
        };

        let error = delete_path_smart(&target, &[dir.path()], &recycler, &stop).unwrap_err();

        assert_eq!(
            *recycler.calls.borrow(),
            [target.clone(), target.join("f0000")],
            "the parent is not tried again after Stop"
        );
        assert!(target.is_dir());
        assert!(error.to_string().contains("stopped before everything"));
    }

    /// Moves like [`MockRecycler`], and after the first move runs `change`,
    /// the way another program writes while a cleanup runs.
    struct ChangingRecycler<F: Fn()> {
        inner: MockRecycler,
        change: F,
    }

    impl<F: Fn()> Recycler for ChangingRecycler<F> {
        fn recycle(&self, paths: &[&Path]) -> Result<(), RecycleError> {
            let first = self.inner.calls.borrow().is_empty();
            let result = self.inner.recycle(paths);
            if first {
                (self.change)();
            }
            result
        }
    }

    fn filtered_cache(dir: &Path) -> (Plan, PathBuf, PathBuf) {
        let cache = dir.join("cache");
        create_dir_all(&cache).unwrap();
        let old = SystemTime::now() - std::time::Duration::from_hours(10 * 24);
        for name in ["a", "b"] {
            let file = File::create(cache.join(name)).unwrap();
            file.set_len(5).unwrap();
            file.set_modified(old).unwrap();
        }
        let mut target = Target::whole(cache.clone(), 10);
        target.keep_folder = true;
        let plan = Plan {
            groups: vec![sized_group("App", vec![target])],
            ..Plan::default()
        };
        (plan, cache.join("a"), cache.join("b"))
    }

    #[test]
    fn a_file_rewritten_during_a_filtered_cleanup_stays() {
        let dir = tempfile::tempdir().unwrap();
        let (mut plan, first, second) = filtered_cache(dir.path());
        let recycler = ChangingRecycler {
            inner: MockRecycler::new(),
            change: || {
                File::options()
                    .write(true)
                    .open(&second)
                    .unwrap()
                    .set_modified(SystemTime::now())
                    .unwrap();
            },
        };
        let rules = Rules {
            min_age_days: 1,
            ..Rules::default()
        };

        run_with(
            &mut plan,
            dir.path(),
            &recycler,
            &rules,
            &AtomicBool::new(false),
        );

        assert!(!first.exists());
        assert!(
            second.exists(),
            "a file changed since it was planned is new"
        );
        assert_eq!(recycler.inner.call_sizes(), [1]);
    }

    #[test]
    fn a_file_replaced_by_a_folder_during_a_filtered_cleanup_stays() {
        let dir = tempfile::tempdir().unwrap();
        let (mut plan, first, second) = filtered_cache(dir.path());
        let recycler = ChangingRecycler {
            inner: MockRecycler::new(),
            change: || {
                fs::remove_file(&second).unwrap();
                create_dir_all(&second).unwrap();
                fs::write(second.join("new"), b"x").unwrap();
            },
        };
        let rules = Rules {
            min_size_bytes: 1,
            ..Rules::default()
        };

        run_with(
            &mut plan,
            dir.path(),
            &recycler,
            &rules,
            &AtomicBool::new(false),
        );

        assert!(!first.exists());
        assert!(second.join("new").exists(), "the folder is not moved whole");
        assert_eq!(recycler.inner.call_sizes(), [1]);
    }

    fn selected_group(app: &str, paths: Vec<PathBuf>) -> Group {
        Group {
            targets: paths
                .into_iter()
                .map(|path| Target::whole(path, 5))
                .collect(),
            bytes: 10,
            on: true,
            ..Group::new(app, "test")
        }
    }

    /// Refuses every move, the way a Recycle Bin that cannot take the files does.
    struct RefusingRecycler;

    impl Recycler for RefusingRecycler {
        fn recycle(&self, _paths: &[&Path]) -> Result<(), RecycleError> {
            Err(RecycleError::ShellOperation {
                code: 5,
                message: String::new(),
            })
        }
    }

    fn sized_group(app: &str, targets: Vec<Target>) -> Group {
        Group {
            bytes: targets.iter().map(|target| target.bytes).sum(),
            targets,
            on: true,
            ..Group::new(app, "test")
        }
    }

    fn execute_plain(
        plan: &Plan,
        dry_run: bool,
        roots: &[&Path],
        recycler: &dyn Recycler,
        progress: impl FnMut(ProgressUpdate),
    ) -> ExecResult {
        let stop = AtomicBool::new(false);
        let rules = Rules::default();
        let run = Run {
            options: Options { dry_run },
            guard_roots: roots,
            rules: &rules,
            stop: &stop,
            is_running: &|_| false,
        };
        execute_with_result(plan, &run, recycler, progress)
    }

    fn run(plan: &mut Plan, root: &Path, recycler: &dyn Recycler) -> ExecResult {
        run_with(
            plan,
            root,
            recycler,
            &Rules::default(),
            &AtomicBool::new(false),
        )
    }

    fn run_with(
        plan: &mut Plan,
        root: &Path,
        recycler: &dyn Recycler,
        rules: &Rules,
        stop: &AtomicBool,
    ) -> ExecResult {
        plan.recompute_totals();
        let roots = [root];
        let run = Run {
            options: Options { dry_run: false },
            guard_roots: &roots,
            rules,
            stop,
            is_running: &|_| false,
        };
        execute_with_result(plan, &run, recycler, |_| {})
    }

    /// Moves paths like [`MockRecycler`] and sets the stop flag after the
    /// first one, the way the user's Stop click lands mid-run.
    struct StoppingRecycler<'a> {
        inner: MockRecycler,
        stop: &'a AtomicBool,
    }

    impl Recycler for StoppingRecycler<'_> {
        fn recycle(&self, paths: &[&Path]) -> Result<(), RecycleError> {
            let result = self.inner.recycle(paths);
            self.stop.store(true, Ordering::Relaxed);
            result
        }
    }

    fn files(root: &Path, entries: &[(&str, usize)]) {
        for (name, size) in entries {
            let path = root.join(name);
            create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, vec![0; *size]).unwrap();
        }
    }

    #[test]
    fn an_exclusion_added_after_the_scan_still_protects_its_path() {
        let dir = tempfile::tempdir().unwrap();
        let cache = dir.path().join("cache");
        files(&cache, &[("a", 10), ("keep/precious", 20), ("other/b", 5)]);
        // Scanned with no exclusions: the whole folder was planned.
        let mut plan = Plan {
            groups: vec![sized_group("App", vec![Target::whole(cache.clone(), 35)])],
            ..Plan::default()
        };
        let rules = Rules {
            exclusions: vec![cache.join("keep")],
            ..Rules::default()
        };

        let mock = MockRecycler::new();
        let result = run_with(
            &mut plan,
            dir.path(),
            &mock,
            &rules,
            &AtomicBool::new(false),
        );
        assert!(
            cache.join("keep/precious").exists(),
            "the exclusion was bypassed"
        );
        assert!(!cache.join("a").exists() && !cache.join("other").exists());
        assert!(
            mock.calls
                .borrow()
                .iter()
                .flatten()
                .all(|path| path != &cache),
            "a folder holding an exclusion is never moved whole"
        );
        assert_eq!(result.freed_bytes, Some(15));
        assert_eq!(result.error_count, 0);
    }

    #[test]
    fn an_excluded_target_and_a_version_holding_one_stay_whole() {
        let dir = tempfile::tempdir().unwrap();
        let cache = dir.path().join("cache");
        files(&cache, &[("a", 1)]);
        let agent = dir.path().join("Agent");
        files(
            &agent,
            &[
                ("Agent.9700/Agent.exe", 1),
                ("Agent.9700/data/x", 1),
                ("Agent.9852/Agent.exe", 1),
            ],
        );
        fs::write(agent.join(crate::versions::PRODUCT_DB), b"x2.41.2.9852R").unwrap();
        let rule = crate::catalog::Versioned {
            dir: agent.clone(),
            prefix: "Agent.".to_owned(),
        };
        let old = agent.join("Agent.9700");
        let mut plan = Plan {
            groups: vec![
                sized_group("App", vec![Target::whole(cache.clone(), 1)]),
                sized_group(
                    "Battle.net",
                    vec![Target {
                        version_rule: Some(rule),
                        ..Target::whole(old.clone(), 2)
                    }],
                ),
            ],
            ..Plan::default()
        };
        let rules = Rules {
            exclusions: vec![cache.clone(), old.join("data/x")],
            min_size_bytes: 1,
            ..Rules::default()
        };

        let mock = MockRecycler::new();
        run_with(
            &mut plan,
            dir.path(),
            &mock,
            &rules,
            &AtomicBool::new(false),
        );
        assert!(mock.calls.borrow().is_empty());
        assert!(cache.join("a").exists());
        assert!(
            old.join("Agent.exe").exists(),
            "a program version is never cleaned in part"
        );
    }

    #[test]
    fn a_size_limit_moves_only_large_files_and_keeps_folders() {
        let dir = tempfile::tempdir().unwrap();
        let cache = dir.path().join("cache");
        files(&cache, &[("small", 10), ("sub/big", 5000)]);
        let mut plan = Plan {
            groups: vec![sized_group("App", vec![Target::whole(cache.clone(), 5010)])],
            ..Plan::default()
        };
        let rules = Rules {
            min_size_bytes: 1000,
            ..Rules::default()
        };
        let mock = MockRecycler::new();
        let result = run_with(
            &mut plan,
            dir.path(),
            &mock,
            &rules,
            &AtomicBool::new(false),
        );
        assert!(cache.join("small").exists());
        assert!(cache.join("sub").is_dir());
        assert!(!cache.join("sub/big").exists());
        assert_eq!(result.freed_bytes, Some(5000));
    }

    #[test]
    fn stop_finishes_the_current_path_and_starts_nothing_else() {
        let dir = tempfile::tempdir().unwrap();
        let temp = dir.path().join("Temp");
        files(&temp, &[("a", 1), ("b", 1), ("c", 1)]);
        let later = dir.path().join("later");
        files(&later, &[("x", 1)]);
        let mut plan = Plan {
            groups: vec![
                sized_group(
                    "A",
                    vec![Target {
                        keep_folder: true,
                        ..Target::whole(temp.clone(), 3)
                    }],
                ),
                sized_group("B", vec![Target::whole(later.clone(), 1)]),
            ],
            ..Plan::default()
        };
        let stop = AtomicBool::new(false);
        let recycler = StoppingRecycler {
            inner: MockRecycler::new(),
            stop: &stop,
        };
        let result = run_with(&mut plan, dir.path(), &recycler, &Rules::default(), &stop);
        assert!(result.stopped);
        assert_eq!(
            recycler.inner.call_sizes(),
            vec![1],
            "one path, then the stop"
        );
        assert!(!temp.join("a").exists());
        assert!(temp.join("b").exists() && temp.join("c").exists());
        assert!(later.join("x").exists());
        assert_eq!(result.groups.len(), 1, "the second group never started");
        assert!(result.groups[0].stopped, "Stop cut the first group short");
        assert_eq!(result.freed_bytes, Some(1));
    }

    #[test]
    fn a_running_program_skips_its_target() {
        let dir = tempfile::tempdir().unwrap();
        let cache = dir.path().join("cache");
        files(&cache, &[("a", 1)]);
        let mut group = sized_group("Discord", vec![Target::whole(cache.clone(), 1)]);
        group.processes = vec!["Discord.exe".to_owned()];
        let mut plan = Plan {
            groups: vec![group],
            ..Plan::default()
        };
        plan.recompute_totals();
        let rules = Rules {
            skip_running_apps: true,
            ..Rules::default()
        };
        let stop = AtomicBool::new(false);
        let roots = [dir.path()];
        let mock = MockRecycler::new();
        let run = Run {
            options: Options { dry_run: false },
            guard_roots: &roots,
            rules: &rules,
            stop: &stop,
            is_running: &|name| name.eq_ignore_ascii_case("discord.exe"),
        };
        let result = execute_with_result(&plan, &run, &mock, |_| {});
        assert!(cache.join("a").exists());
        assert_eq!(
            result.groups[0].skipped.as_deref(),
            Some("Discord.exe is running")
        );
        assert!(mock.calls.borrow().is_empty());

        let unchecked = Rules::default();
        let run = Run {
            rules: &unchecked,
            ..run
        };
        execute_with_result(&plan, &run, &mock, |_| {});
        assert!(!cache.exists(), "without the setting the target is cleaned");
    }

    #[test]
    fn a_failed_cleanup_records_nothing_freed() {
        let dir = tempfile::tempdir().unwrap();
        let cache = dir.path().join("cache");
        make_children(&cache, 2);
        fs::write(cache.join("f0000"), [0; 700]).unwrap();
        let mut plan = Plan {
            groups: vec![sized_group("App", vec![Target::whole(cache.clone(), 700)])],
            ..Plan::default()
        };

        let result = run(&mut plan, dir.path(), &RefusingRecycler);
        assert_eq!(result.total_bytes, 700, "the selection is still recorded");
        assert_eq!(result.freed_bytes, Some(0));
        assert_eq!(result.freed(), 0);
        assert_eq!(result.groups[0].freed, Some(0));
        assert!(cache.exists());
    }

    #[test]
    fn freed_bytes_count_what_left_and_not_what_stayed() {
        let dir = tempfile::tempdir().unwrap();
        let cache = dir.path().join("cache");
        create_dir_all(&cache).unwrap();
        fs::write(cache.join("locked"), [0; 300]).unwrap();
        fs::write(cache.join("free"), [0; 500]).unwrap();
        let mut recycler = FailingRecycler::new();
        recycler.fail_always.insert(cache.clone());
        recycler.fail_always.insert(cache.join("locked"));
        let mut plan = Plan {
            groups: vec![sized_group("App", vec![Target::whole(cache.clone(), 800)])],
            ..Plan::default()
        };

        let result = run(&mut plan, dir.path(), &recycler);
        assert_eq!(result.groups[0].paths_failed, 1);
        assert_eq!(result.freed_bytes, Some(500));
    }

    #[test]
    fn a_kept_folder_loses_only_what_it_holds() {
        let dir = tempfile::tempdir().unwrap();
        let temp = dir.path().join("Temp");
        make_children(&temp, 3);
        create_dir_all(temp.join("installer")).unwrap();
        let mut plan = Plan {
            groups: vec![sized_group(
                "Windows",
                vec![Target {
                    keep_folder: true,
                    ..Target::whole(temp.clone(), 0)
                }],
            )],
            ..Plan::default()
        };

        let mock = MockRecycler::new();
        let result = run(&mut plan, dir.path(), &mock);
        assert_eq!(result.error_count, 0);
        assert!(temp.is_dir(), "the folder itself must stay");
        assert_eq!(fs::read_dir(&temp).unwrap().count(), 0);
        assert!(
            mock.calls
                .borrow()
                .iter()
                .flatten()
                .all(|path| path != &temp),
            "the folder was never submitted"
        );
    }

    #[test]
    fn a_locked_file_in_a_kept_folder_leaves_the_rest_recycled() {
        let dir = tempfile::tempdir().unwrap();
        let temp = dir.path().join("Temp");
        make_children(&temp, 3);
        let mut recycler = FailingRecycler::new();
        recycler.fail_always.insert(temp.join("f0001"));
        let mut plan = Plan {
            groups: vec![sized_group(
                "Windows",
                vec![Target {
                    keep_folder: true,
                    ..Target::whole(temp.clone(), 0)
                }],
            )],
            ..Plan::default()
        };

        let result = run(&mut plan, dir.path(), &recycler);
        assert_eq!(result.error_count, 1);
        assert!(result.groups[0].errors[0].path.ends_with("f0001"));
        assert!(temp.join("f0001").exists());
        assert!(!temp.join("f0000").exists() && !temp.join("f0002").exists());
        assert!(!recycler.calls().contains(&temp));
    }

    #[test]
    fn a_kept_folder_must_be_a_real_folder() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("file");
        fs::write(&file, b"x").unwrap();
        let mut plan = Plan {
            groups: vec![sized_group(
                "Windows",
                vec![Target {
                    keep_folder: true,
                    ..Target::whole(file.clone(), 1)
                }],
            )],
            ..Plan::default()
        };
        let mock = MockRecycler::new();
        let result = run(&mut plan, dir.path(), &mock);
        assert_eq!(result.error_count, 1);
        assert!(mock.calls.borrow().is_empty());
        assert!(file.exists());
    }

    #[test]
    fn a_version_installed_since_the_scan_is_left_alone() {
        let dir = tempfile::tempdir().unwrap();
        let agent = dir.path().join("Agent");
        for name in ["Agent.9700", "Agent.9852"] {
            make_children(&agent.join(name), 1);
        }
        fs::write(agent.join(crate::versions::PRODUCT_DB), b"x2.41.2.9852R").unwrap();
        let rule = crate::catalog::Versioned {
            dir: agent.clone(),
            prefix: "Agent.".to_owned(),
        };
        let old = agent.join("Agent.9700");
        let mut plan = Plan {
            groups: vec![sized_group(
                "Battle.net",
                vec![Target {
                    version_rule: Some(rule),
                    ..Target::whole(old.clone(), 1)
                }],
            )],
            ..Plan::default()
        };
        // After the scan, the updater rolled back to the older build.
        fs::write(agent.join(crate::versions::PRODUCT_DB), b"x2.41.1.9700R").unwrap();

        let mock = MockRecycler::new();
        let result = run(&mut plan, dir.path(), &mock);
        assert!(old.exists(), "the build now installed must survive");
        assert_eq!(result.groups[0].paths_attempted, 0);
        assert!(mock.calls.borrow().is_empty());
    }

    #[test]
    fn a_version_that_cannot_move_whole_stays_whole() {
        let dir = tempfile::tempdir().unwrap();
        let agent = dir.path().join("Agent");
        for name in ["Agent.9700", "Agent.9852"] {
            make_children(&agent.join(name), 3);
        }
        fs::write(agent.join(crate::versions::PRODUCT_DB), b"x2.41.2.9852R").unwrap();
        let old = agent.join("Agent.9700");
        for file in fs::read_dir(&old).unwrap() {
            fs::write(file.unwrap().path(), b"x").unwrap();
        }
        let mut plan = Plan {
            groups: vec![sized_group(
                "Battle.net",
                vec![Target {
                    version_rule: Some(crate::catalog::Versioned {
                        dir: agent.clone(),
                        prefix: "Agent.".to_owned(),
                    }),
                    ..Target::whole(old.clone(), 3)
                }],
            )],
            ..Plan::default()
        };
        // One file inside is locked, so the folder cannot move whole.
        let mut recycler = FailingRecycler::new();
        recycler.fail_always.insert(old.clone());

        let result = run(&mut plan, dir.path(), &recycler);

        assert_eq!(
            recycler.calls(),
            std::slice::from_ref(&old),
            "no file inside is tried"
        );
        assert_eq!(fs::read_dir(&old).unwrap().count(), 3);
        assert_eq!(result.groups[0].paths_failed, 1);
        assert_eq!(result.groups[0].freed, Some(0));
    }

    #[test]
    fn execute_reguards_paths_and_records_results() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("safe");
        let inside = root.join("app-cache");
        make_children(&inside, 2);
        let outside = dir.path().join("outside");
        create_dir_all(&outside).unwrap();

        let mut plan = Plan {
            groups: vec![
                selected_group("Good", vec![inside.clone()]),
                selected_group("Evil", vec![outside.clone()]),
                selected_group("Gone", vec![root.join("never-existed")]),
            ],
            ..Plan::default()
        };
        plan.recompute_totals();

        let mock = MockRecycler::new();
        let mut updates = Vec::new();
        let result = execute_plain(&plan, false, &[&root], &mock, |update| updates.push(update));

        assert_eq!(result.groups.len(), 3);
        let good = &result.groups[0];
        assert_eq!(good.paths_attempted, 1);
        assert_eq!(good.paths_failed, 0);
        assert!(!inside.exists());

        let evil = &result.groups[1];
        assert_eq!(evil.paths_attempted, 0);
        assert_eq!(evil.errors.len(), 1);
        assert_eq!(evil.errors[0].error, "unsafe path (guard)");
        assert!(outside.exists(), "unsafe path must never be touched");

        let gone = &result.groups[2];
        assert_eq!(gone.paths_attempted, 0);
        assert!(gone.errors.is_empty());

        assert_eq!(result.error_count, 1);
        assert_eq!(updates.len(), 3);
    }

    #[cfg(unix)]
    #[test]
    fn execute_rejects_paths_through_symlink_ancestors() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("safe");
        let outside = dir.path().join("outside");
        make_children(&outside.join("cache"), 1);
        create_dir_all(&root).unwrap();
        std::os::unix::fs::symlink(&outside, root.join("linked")).unwrap();
        let escaped = root.join("linked/cache");

        let mut plan = Plan {
            groups: vec![selected_group("Unsafe", vec![escaped])],
            ..Plan::default()
        };
        plan.recompute_totals();

        let mock = MockRecycler::new();
        let result = execute_plain(&plan, false, &[&root], &mock, |_| {});

        assert_eq!(mock.call_sizes(), Vec::<usize>::new());
        assert!(outside.join("cache/f0000").exists());
        assert_eq!(result.error_count, 1);
        assert_eq!(result.groups[0].errors[0].error, "unsafe path (guard)");
    }

    #[test]
    fn empty_folder_cleanup_removes_only_still_empty_directories() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("safe");
        let empty = root.join("empty");
        let changed = root.join("changed");
        create_dir_all(&empty).unwrap();
        create_dir_all(&changed).unwrap();
        File::create(changed.join("appeared-after-scan.txt")).unwrap();

        let mut plan = Plan {
            groups: vec![selected_group(
                EMPTY_FOLDERS_APP,
                vec![empty.clone(), changed.clone()],
            )],
            ..Plan::default()
        };
        plan.recompute_totals();

        let mock = MockRecycler::new();
        let result = execute_plain(&plan, false, &[&root], &mock, |_| {});

        assert!(!empty.exists());
        assert!(changed.join("appeared-after-scan.txt").exists());
        assert_eq!(mock.call_sizes(), Vec::<usize>::new());
        assert_eq!(result.groups[0].paths_attempted, 2);
        assert_eq!(result.groups[0].paths_failed, 1);
        assert_eq!(result.error_count, 1);
    }

    #[test]
    fn dry_run_touches_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("safe");
        let inside = root.join("cache");
        make_children(&inside, 2);

        let mut plan = Plan {
            groups: vec![selected_group("Good", vec![inside.clone()])],
            ..Plan::default()
        };
        plan.recompute_totals();

        let mock = MockRecycler::new();
        let result = execute_plain(&plan, true, &[&root], &mock, |_| {});
        assert_eq!(mock.call_sizes(), Vec::<usize>::new());
        assert!(inside.exists());
        assert!(result.dry_run);
        assert!(result.groups.is_empty());
    }

    /// Deletes what it is given for good and says so, the way the shell does
    /// when the Recycle Bin is off for a drive. Paths in `refuse` are refused
    /// and left in place instead.
    struct DestroyingRecycler {
        calls: RefCell<Vec<PathBuf>>,
        refuse: HashSet<PathBuf>,
    }

    impl DestroyingRecycler {
        fn new(refuse: &[&Path]) -> Self {
            Self {
                calls: RefCell::new(Vec::new()),
                refuse: refuse.iter().map(|path| path.to_path_buf()).collect(),
            }
        }
    }

    impl Recycler for DestroyingRecycler {
        fn recycle(&self, paths: &[&Path]) -> Result<(), RecycleError> {
            assert_eq!(paths.len(), 1);
            let path = paths[0];
            self.calls.borrow_mut().push(path.to_path_buf());
            if self.refuse.contains(path) {
                return Err(RecycleError::ShellOperation {
                    code: 32,
                    message: String::new(),
                });
            }
            if path.is_dir() {
                fs::remove_dir_all(path).unwrap();
            } else {
                fs::remove_file(path).unwrap();
            }
            Err(RecycleError::NotRecycled)
        }
    }

    /// Refuses every move the way the shell does when the Recycle Bin is off
    /// or too small for the drive, leaving each path in place.
    struct BinOffRecycler {
        calls: RefCell<Vec<PathBuf>>,
    }

    impl Recycler for BinOffRecycler {
        fn recycle(&self, paths: &[&Path]) -> Result<(), RecycleError> {
            self.calls
                .borrow_mut()
                .extend(paths.iter().map(|path| path.to_path_buf()));
            Err(RecycleError::RecycleBinOff)
        }
    }

    #[test]
    fn a_refused_move_ends_the_run_without_salvaging_children() {
        let dir = tempfile::tempdir().unwrap();
        let first = dir.path().join("first");
        let second = dir.path().join("second");
        files(&first, &[("a", 10), ("b", 5)]);
        files(&second, &[("c", 20)]);
        let mut plan = Plan {
            groups: vec![
                sized_group("A", vec![Target::whole(first.clone(), 15)]),
                sized_group("B", vec![Target::whole(second.clone(), 20)]),
            ],
            ..Plan::default()
        };
        let recycler = BinOffRecycler {
            calls: RefCell::new(Vec::new()),
        };
        let result = run(&mut plan, dir.path(), &recycler);

        assert!(first.join("a").exists() && first.join("b").exists());
        assert!(second.join("c").exists());
        // Only the folder itself was tried: no child, no later target.
        assert_eq!(*recycler.calls.borrow(), vec![first.clone()]);
        assert!(result.stopped);
        assert!(result.recycle_bin_off());
        assert_eq!(result.groups.len(), 1);
        let group = &result.groups[0];
        assert!(group.recycle_bin_off);
        assert_eq!(group.not_recycled, 0);
        assert_eq!(group.freed, Some(0));
        assert_eq!(
            result.cleaned_count(),
            0,
            "a target that only failed was not cleaned"
        );
        assert_eq!(
            group.errors[0].error,
            RecycleError::RecycleBinOff.to_string()
        );
    }

    #[test]
    fn a_permanent_delete_is_reported_and_ends_the_run() {
        let dir = tempfile::tempdir().unwrap();
        let first = dir.path().join("first");
        let second = dir.path().join("second");
        files(&first, &[("a", 10)]);
        files(&second, &[("b", 20)]);
        let mut plan = Plan {
            groups: vec![
                sized_group("A", vec![Target::whole(first.clone(), 10)]),
                sized_group("B", vec![Target::whole(second.clone(), 20)]),
            ],
            ..Plan::default()
        };
        let recycler = DestroyingRecycler::new(&[]);
        let result = run(&mut plan, dir.path(), &recycler);

        // The path is gone, but it is not reported as moved or freed.
        assert!(!first.exists());
        assert!(result.stopped, "nothing after the permanent delete runs");
        assert_eq!(result.groups.len(), 1);
        assert!(second.join("b").exists());
        assert_eq!(*recycler.calls.borrow(), vec![first.clone()]);
        let group = &result.groups[0];
        assert_eq!(group.not_recycled, 1);
        assert_eq!(group.paths_failed, 1);
        assert_eq!(group.freed, Some(0));
        assert_eq!(result.freed_bytes, Some(0));
        assert_eq!(result.not_recycled_count(), 1);
        assert_eq!(result.error_count, 1);
        assert_eq!(group.errors[0].path, first.display().to_string());
        assert_eq!(
            group.errors[0].error,
            "permanently deleted, not in the Recycle Bin"
        );
    }

    #[test]
    fn a_permanent_delete_of_a_salvaged_child_stops_the_salvage() {
        let dir = tempfile::tempdir().unwrap();
        let cache = dir.path().join("cache");
        make_children(&cache, 3);
        let recycler = DestroyingRecycler::new(&[&cache]);
        let mut destroyed = Vec::new();
        let outcome = super::delete_path_smart(
            &cache,
            &[dir.path()],
            &recycler,
            &AtomicBool::new(false),
            &mut destroyed,
        );

        assert!(matches!(outcome, Err(RecycleError::NotRecycled)));
        assert_eq!(destroyed.len(), 1);
        let calls = recycler.calls.borrow();
        assert_eq!(
            *calls,
            vec![cache.clone(), destroyed[0].clone()],
            "no other child is tried and the folder is not retried"
        );
        assert_eq!(fs::read_dir(&cache).unwrap().count(), 2);
    }

    #[test]
    fn a_permanent_delete_of_a_whole_version_is_reported() {
        let dir = tempfile::tempdir().unwrap();
        let version = dir.path().join("Agent.1");
        files(&version, &[("agent.exe", 10)]);
        let recycler = DestroyingRecycler::new(&[]);
        let mut destroyed = Vec::new();
        let outcome = move_whole(&version, &[dir.path()], &recycler, &mut destroyed);
        assert!(matches!(outcome, Err(RecycleError::NotRecycled)));
        assert_eq!(destroyed, vec![version]);
    }

    #[test]
    fn salvage_leaves_names_windows_would_trim() {
        let dir = tempfile::tempdir().unwrap();
        let cache = dir.path().join("cache");
        files(&cache, &[("a", 1)]);
        // Windows would move `a` when asked for `a.`.
        let trimmed = cache.join("a.");
        fs::write(crate::rules::verbatim_for_test(&trimmed), b"x").unwrap();
        let mut recycler = FailingRecycler::new();
        recycler.fail_always.insert(cache.clone());

        let error = delete_path_smart(&cache, &[dir.path()], &recycler, &AtomicBool::new(false))
            .unwrap_err()
            .to_string();
        assert!(error.contains("ends in a dot or a space"), "{error}");
        assert_eq!(
            recycler.calls(),
            vec![cache.clone(), cache.join("a"), cache.clone()]
        );
        assert!(fs::symlink_metadata(crate::rules::verbatim_for_test(&trimmed)).is_ok());
    }

    #[test]
    fn the_folders_from_the_outermost_root_down_to_the_parent_are_held() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("root");
        let target = root.join("a/b/file.bin");
        files(&root, &[("a/b/file.bin", 1)]);
        let inner = root.join("a");

        assert_eq!(hold_parents(&target, &[&root]).unwrap().len(), 3);
        assert_eq!(
            hold_parents(&target, &[&inner, &root]).unwrap().len(),
            3,
            "the outermost root that holds the path is where the hold starts"
        );
        assert_eq!(hold_parents(&target, &[&inner]).unwrap().len(), 2);
        assert_eq!(
            hold_parents(&dir.path().join("elsewhere/x"), &[&root])
                .unwrap()
                .len(),
            0,
            "a path under no root is left to the guard"
        );
    }

    #[cfg(windows)]
    #[test]
    fn a_held_folder_cannot_be_renamed_until_the_move_ends() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("root");
        files(&root, &[("cache/sub/file.bin", 1)]);
        let held = hold_parents(&root.join("cache/sub/file.bin"), &[&root]).unwrap();
        // A same-user program swapping `cache` for a junction must rename it
        // first, and the hold refuses that.
        assert!(fs::rename(root.join("cache"), root.join("moved")).is_err());
        drop(held);
        fs::rename(root.join("cache"), root.join("moved")).unwrap();
    }

    #[cfg(windows)]
    #[test]
    fn a_held_parent_still_lets_its_child_move() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("root");
        files(&root, &[("cache/file.bin", 1)]);
        let _held = hold_parents(&root.join("cache/file.bin"), &[&root]).unwrap();
        // The shell renames the child into the Recycle Bin. A rename within
        // the held folder stands in for that here.
        fs::rename(root.join("cache/file.bin"), root.join("cache/renamed.bin")).unwrap();
    }

    #[cfg(windows)]
    #[test]
    fn holding_a_missing_folder_fails_and_moves_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("root");
        create_dir_all(&root).unwrap();
        let missing = root.join("gone/file.bin");
        assert_eq!(
            hold_parents(&missing, &[&root]).unwrap_err().kind(),
            std::io::ErrorKind::NotFound
        );
        let recycler = MockRecycler::new();
        assert!(matches!(
            guarded_recycle(&missing, &[&root], &recycler),
            Err(RecycleError::ParentNotHeld { .. })
        ));
        assert_eq!(recycler.call_sizes(), Vec::<usize>::new());
    }
}
