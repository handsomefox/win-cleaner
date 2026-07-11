//! Plan execution: moves selected paths to the Recycle Bin via a [`Recycler`]
//! implementation. The safety guard is re-checked immediately before every
//! delete, independent of the scan-time check.

use std::fs;
use std::path::{Path, PathBuf};

use crate::error::RecycleError;
use crate::plan::{Options, Phase, Plan, ProgressUpdate};
use crate::safety::is_safe_path;
use crate::stats::{ExecResult, GroupResult, PathError};

/// Number of immediate children above which we attempt to move the whole
/// directory in one shot rather than batching children. Shader caches and
/// Battle.net folders can have tens of thousands of files; individually
/// recycling hundreds of chunks of 64 is extremely slow on Windows.
const LARGE_CHILD_THRESHOLD: usize = 500;

/// Number of paths sent per recycler call when processing directories below
/// the large-child threshold.
const CHUNK_SIZE: usize = 64;

/// Moves paths to the Recycle Bin. The single production implementation wraps
/// `SHFileOperationW` in `cleaner-platform`; tests use mocks.
pub trait Recycler {
    /// Moves one or more absolute paths to the Recycle Bin in a single call.
    ///
    /// # Errors
    ///
    /// Returns an error when the shell refuses or aborts the operation.
    fn recycle(&self, paths: &[&Path]) -> Result<(), RecycleError>;
}

/// Executes the selected groups of `plan`, re-guarding every path against
/// `guard_roots`. As a defense in depth (the GUI never calls this in preview
/// mode) a dry run performs no deletions and reports no groups.
pub fn execute_with_result(
    plan: &Plan,
    opts: Options,
    guard_roots: &[&Path],
    recycler: &dyn Recycler,
    mut progress: impl FnMut(ProgressUpdate),
) -> ExecResult {
    let mut result = ExecResult::begin(plan, opts);
    if plan.selected == 0 || opts.dry_run {
        result.finish();
        return result;
    }

    let total = plan.groups.iter().filter(|group| group.on).count();
    for (index, group) in plan.groups.iter().filter(|group| group.on).enumerate() {
        progress(ProgressUpdate {
            phase: Phase::Delete,
            current: index + 1,
            total,
            message: format!("{} - {}", group.app, group.label),
        });

        let mut group_result = GroupResult {
            app: group.app.clone(),
            label: group.label.clone(),
            errors: Vec::new(),
            bytes: group.bytes,
            paths_attempted: 0,
            paths_failed: 0,
        };
        for path in &group.paths {
            if !is_safe_path(path, guard_roots) {
                group_result.errors.push(PathError {
                    path: path.display().to_string(),
                    error: "unsafe path (guard)".to_owned(),
                });
                continue;
            }
            if fs::symlink_metadata(path).is_err() {
                continue;
            }
            group_result.paths_attempted += 1;
            if let Err(err) = delete_path_smart(path, recycler) {
                group_result.paths_failed += 1;
                group_result.errors.push(PathError {
                    path: path.display().to_string(),
                    error: err.to_string(),
                });
            }
        }
        result.error_count += group_result.errors.len();
        result.groups.push(group_result);
    }
    result.finish();
    result
}

/// Deletes `path` using only the Recycle Bin:
/// - File/link: recycle directly.
/// - Small directory (≤ [`LARGE_CHILD_THRESHOLD`] children): batch children
///   in chunks, then recycle the now-empty parent.
/// - Large directory: try recycling the whole folder in one call first (much
///   faster); fall back to the chunk approach only if that fails.
///
/// # Errors
///
/// Returns an error when the recycler rejects a path; nothing is ever
/// deleted permanently.
pub fn delete_path_smart(path: &Path, recycler: &dyn Recycler) -> Result<(), RecycleError> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        // If we can't stat it, still try the Recycle Bin (may succeed).
        Err(_) => return recycler.recycle(&[path]),
    };

    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return recycler.recycle(&[path]);
    }

    let children = match read_dir_paths(path) {
        Ok(children) => children,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        // Enumeration failed (permission issues); try the folder itself and
        // report the recycler's result to the caller.
        Err(_) => return recycler.recycle(&[path]),
    };

    // Large directory: one shell call for the whole tree when possible.
    if children.len() > LARGE_CHILD_THRESHOLD && recycler.recycle(&[path]).is_ok() {
        return Ok(());
    }

    for chunk in children.chunks(CHUNK_SIZE) {
        let refs: Vec<&Path> = chunk.iter().map(PathBuf::as_path).collect();
        if recycler.recycle(&refs).is_err() {
            // Bulk move failed; attempt per-item to salvage progress.
            let mut item_errors = Vec::new();
            for child in chunk {
                if let Err(err) = recycler.recycle(&[child]) {
                    item_errors.push(format!("{}: {err}", child.display()));
                }
            }
            if !item_errors.is_empty() {
                return Err(RecycleError::Multiple(item_errors.join("; ")));
            }
        }
    }

    // Recycle the (now hopefully empty) directory itself.
    recycler.recycle(&[path])
}

/// Absolute paths of the immediate children of `dir`.
fn read_dir_paths(dir: &Path) -> std::io::Result<Vec<PathBuf>> {
    let mut out: Vec<PathBuf> = fs::read_dir(dir)?
        .filter_map(Result::ok)
        .map(|entry| dir.join(entry.file_name()))
        .collect();
    out.sort();
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan::Group;
    use std::cell::RefCell;
    use std::fs::{File, create_dir_all};

    /// Records every batch and optionally fails calls; successful calls
    /// actually remove the paths so "recycle the now-empty parent" works.
    struct MockRecycler {
        calls: RefCell<Vec<Vec<PathBuf>>>,
        /// Paths this mock refuses to delete.
        refuse: Vec<PathBuf>,
        /// When set, any call with more than one path fails.
        fail_bulk: bool,
        /// When set, whole-directory calls (single non-empty dir path) fail.
        fail_whole_dir: bool,
    }

    impl MockRecycler {
        fn new() -> Self {
            Self {
                calls: RefCell::new(Vec::new()),
                refuse: Vec::new(),
                fail_bulk: false,
                fail_whole_dir: false,
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
            if self.fail_bulk && paths.len() > 1 {
                return Err(RecycleError::ShellOperation(5));
            }
            for path in paths {
                if self.refuse.iter().any(|r| r == *path) {
                    return Err(RecycleError::ShellOperation(32));
                }
                if self.fail_whole_dir
                    && path.is_dir()
                    && fs::read_dir(path).is_ok_and(|mut d| d.next().is_some())
                {
                    return Err(RecycleError::ShellOperation(145));
                }
            }
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
        delete_path_smart(&file, &mock).unwrap();
        assert_eq!(mock.call_sizes(), vec![1]);
        assert!(!file.exists());
    }

    #[test]
    fn missing_path_is_ok_without_calls() {
        let dir = tempfile::tempdir().unwrap();
        let mock = MockRecycler::new();
        delete_path_smart(&dir.path().join("missing"), &mock).unwrap();
        assert!(mock.call_sizes().is_empty());
    }

    #[test]
    fn small_dir_chunks_children_then_parent() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("cache");
        make_children(&target, 100);
        let mock = MockRecycler::new();
        delete_path_smart(&target, &mock).unwrap();
        // 100 children => chunks of 64 + 36, then the parent alone.
        assert_eq!(mock.call_sizes(), vec![64, 36, 1]);
        assert!(!target.exists());
    }

    #[test]
    fn large_dir_recycled_whole_when_possible() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("shadercache");
        make_children(&target, LARGE_CHILD_THRESHOLD + 1);
        let mock = MockRecycler::new();
        delete_path_smart(&target, &mock).unwrap();
        assert_eq!(mock.call_sizes(), vec![1]);
        assert!(!target.exists());
    }

    #[test]
    fn large_dir_falls_back_to_chunks_when_whole_fails() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("locked");
        make_children(&target, LARGE_CHILD_THRESHOLD + 1);
        let mock = MockRecycler {
            fail_whole_dir: true,
            ..MockRecycler::new()
        };
        delete_path_smart(&target, &mock).unwrap();
        let sizes = mock.call_sizes();
        // Whole-dir attempt, then ceil(501/64) = 8 chunks, then parent.
        assert_eq!(sizes[0], 1);
        assert_eq!(sizes.len(), 1 + 8 + 1);
        assert_eq!(*sizes.last().unwrap(), 1);
        assert!(!target.exists());
    }

    #[test]
    fn failed_chunk_salvages_per_item() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("cache");
        make_children(&target, 3);
        let mock = MockRecycler {
            fail_bulk: true,
            refuse: vec![target.join("f0001")],
            ..MockRecycler::new()
        };
        let err = delete_path_smart(&target, &mock).unwrap_err();
        assert!(err.to_string().contains("f0001"));
        // Salvage removed the two other children.
        assert!(!target.join("f0000").exists());
        assert!(!target.join("f0002").exists());
        assert!(target.join("f0001").exists());
    }

    #[cfg(unix)]
    #[test]
    fn enumeration_failure_propagates_recycler_failure() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("unreadable");
        make_children(&target, 1);
        fs::set_permissions(&target, fs::Permissions::from_mode(0o000)).unwrap();

        let mock = MockRecycler {
            refuse: vec![target.clone()],
            ..MockRecycler::new()
        };
        let result = delete_path_smart(&target, &mock);
        fs::set_permissions(&target, fs::Permissions::from_mode(0o755)).unwrap();

        let err = result.unwrap_err();
        assert!(matches!(err, RecycleError::ShellOperation(32)));
        assert_eq!(mock.call_sizes(), vec![1]);
        assert!(target.exists());
    }

    fn selected_group(app: &str, paths: Vec<PathBuf>) -> Group {
        Group {
            app: app.to_owned(),
            label: "test".to_owned(),
            paths,
            errs: Vec::new(),
            bytes: 10,
            on: true,
        }
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
        let result = execute_with_result(
            &plan,
            Options { dry_run: false },
            &[&root],
            &mock,
            |update| updates.push(update),
        );

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
        assert!(updates.iter().all(|u| u.phase == Phase::Delete));
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
        let result =
            execute_with_result(&plan, Options { dry_run: false }, &[&root], &mock, |_| {});

        assert!(mock.call_sizes().is_empty());
        assert!(outside.join("cache/f0000").exists());
        assert_eq!(result.error_count, 1);
        assert_eq!(result.groups[0].errors[0].error, "unsafe path (guard)");
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
        let result = execute_with_result(&plan, Options { dry_run: true }, &[&root], &mock, |_| {});
        assert!(mock.call_sizes().is_empty());
        assert!(inside.exists());
        assert!(result.dry_run);
        assert!(result.groups.is_empty());
    }
}
