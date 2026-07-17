//! Plan execution. Cache paths are moved to the Recycle Bin via a [`Recycler`]
//! implementation. Empty-directory cleanup uses an atomic, non-recursive
//! removal so a directory that gains content after scanning is never deleted.
//! The safety guard is re-checked immediately before every operation.

use std::fs;
use std::path::Path;

use crate::empty_folders::EMPTY_FOLDERS_APP;
use crate::error::RecycleError;
use crate::plan::{Options, Phase, Plan, ProgressUpdate};
use crate::safety::is_safe_path;
use crate::stats::{ExecResult, GroupResult, PathError};

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
            let operation = if group.app == EMPTY_FOLDERS_APP {
                remove_empty_directory(path).map_err(|err| err.to_string())
            } else {
                delete_path_smart(path, recycler).map_err(|err| err.to_string())
            };
            if let Err(error) = operation {
                group_result.paths_failed += 1;
                group_result.errors.push(PathError {
                    path: path.display().to_string(),
                    error,
                });
            }
        }
        result.error_count += group_result.errors.len();
        result.groups.push(group_result);
    }
    result.finish();
    result
}

/// Moves exactly `path` to the Recycle Bin in one operation.
///
/// The core deliberately does not enumerate a directory and submit its
/// children separately. Besides being slower, that would follow a directory
/// junction or other reparse point and expand the operation beyond the path
/// approved by the safety guard.
///
/// # Errors
///
/// Returns an error when the recycler rejects the path.
pub fn delete_path_smart(path: &Path, recycler: &dyn Recycler) -> Result<(), RecycleError> {
    if matches!(
        fs::symlink_metadata(path),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound
    ) {
        return Ok(());
    }
    // If we can't stat it for another reason, still try the Recycle Bin.
    recycler.recycle(&[path])
}

/// Removes an empty directory without recursion. `remove_dir` performs the
/// emptiness check and removal as one filesystem operation, so a directory
/// that gains a file after the scan fails closed instead of deleting it.
fn remove_empty_directory(path: &Path) -> std::io::Result<()> {
    match fs::remove_dir(path) {
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
    use std::fs::{File, create_dir_all};
    use std::path::PathBuf;

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
    fn directory_is_recycled_as_one_top_level_path() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("cache");
        make_children(&target, 100);
        let mock = MockRecycler::new();
        delete_path_smart(&target, &mock).unwrap();
        assert_eq!(mock.call_sizes(), vec![1]);
        assert!(!target.exists());
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
        let result =
            execute_with_result(&plan, Options { dry_run: false }, &[&root], &mock, |_| {});

        assert!(!empty.exists());
        assert!(changed.join("appeared-after-scan.txt").exists());
        assert!(mock.call_sizes().is_empty());
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
        let result = execute_with_result(&plan, Options { dry_run: true }, &[&root], &mock, |_| {});
        assert!(mock.call_sizes().is_empty());
        assert!(inside.exists());
        assert!(result.dry_run);
        assert!(result.groups.is_empty());
    }
}
