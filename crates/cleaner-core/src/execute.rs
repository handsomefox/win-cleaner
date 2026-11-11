//! Plan execution. Cache paths are moved to the Recycle Bin via a [`Recycler`]
//! implementation. Empty-directory cleanup uses an atomic, non-recursive
//! removal so a directory that gains content after scanning is never deleted.
//! The safety guard is re-checked immediately before every operation.

use std::fs;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::SystemTime;

use crate::empty_folders::EMPTY_FOLDERS_APP;
use crate::error::RecycleError;
use crate::plan::{Group, Options, Plan, ProgressUpdate, Target};
use crate::rules::{Rules, Walk, plan_paths};
use crate::safety::{is_link, is_safe_path, normalized_key};
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
        };
        if run.rules.skip_running_apps
            && let Some(name) = group.processes.iter().find(|name| (run.is_running)(name))
        {
            group_result.skipped = Some(format!("{name} is running"));
        } else {
            clean_group(group, run, recycler, &mut group_result);
        }
        if run.stop.load(Ordering::Relaxed) {
            result.stopped = true;
        }
        result.error_count += group_result.errors.len();
        result.freed_bytes = result
            .freed_bytes
            .map(|freed| freed + group_result.freed.unwrap_or(0));
        result.groups.push(group_result);
    }
    result.finish();
    result
}

fn clean_group(group: &Group, run: &Run<'_>, recycler: &dyn Recycler, out: &mut GroupResult) {
    let now = SystemTime::now();
    for target in &group.targets {
        if run.stop.load(Ordering::Relaxed) {
            return;
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
            if let Err(error) = remove_empty_directory(path) {
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
            clean_path(path, target.bytes, run, recycler, out, Salvage::Never);
            continue;
        }
        let whole = !target.keep_folder
            && !run.rules.filters_files()
            && !run.rules.has_exclusion_inside(path);
        if whole {
            clean_path(path, target.bytes, run, recycler, out, Salvage::Children);
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
        let mut walk = Walk::measured();
        plan_paths(
            path,
            target.keep_folder,
            run.rules,
            now,
            &mut walk,
            &mut |child, size| {
                planned.push((child.to_path_buf(), size));
                true
            },
        );
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
                return;
            }
            clean_path(&child, size, run, recycler, out, Salvage::Children);
        }
    }
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
fn clean_path(
    path: &Path,
    planned_bytes: u64,
    run: &Run<'_>,
    recycler: &dyn Recycler,
    out: &mut GroupResult,
    salvage: Salvage,
) {
    out.paths_attempted += 1;
    let moved = match salvage {
        Salvage::Children => delete_path_smart(path, run.guard_roots, recycler, run.stop),
        Salvage::Never => move_whole(path, run.guard_roots, recycler),
    };
    if let Err(error) = moved {
        out.paths_failed += 1;
        out.errors.push(PathError {
            path: path.display().to_string(),
            error: error.to_string(),
        });
    }
    let remaining = if path_missing(path) {
        0
    } else {
        dir_size(path).unwrap_or(planned_bytes)
    };
    out.freed = out
        .freed
        .map(|freed| freed + planned_bytes.saturating_sub(remaining));
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
/// # Errors
///
/// Returns an aggregate error when salvage is incomplete and the directory
/// still exists.
pub fn delete_path_smart(
    path: &Path,
    guard_roots: &[&Path],
    recycler: &dyn Recycler,
    stop: &AtomicBool,
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
        Err(_) => return recycler.recycle(&[path]),
    };
    let is_real_directory = metadata.file_type().is_dir() && !is_link(&metadata);
    let initial_error = match recycler.recycle(&[path]) {
        Ok(()) => return Ok(()),
        Err(_) if path_missing(path) => return Ok(()),
        Err(error) => error,
    };
    if !is_real_directory {
        return Err(initial_error);
    }

    let mut failures = vec![format!("{}: {initial_error}", path.display())];
    if !recycle_children(path, guard_roots, recycler, stop, &mut failures) {
        return if path_missing(path) {
            Ok(())
        } else {
            Err(RecycleError::Multiple(failures.join("; ")))
        };
    }

    if !is_safe_path(path, guard_roots) {
        if path_missing(path) {
            return Ok(());
        }
        failures.push(format!("parent safety guard failed: {}", path.display()));
    } else if let Err(error) = recycler.recycle(&[path])
        && !path_missing(path)
    {
        failures.push(format!("{}: {error}", path.display()));
    }
    if path_missing(path) {
        Ok(())
    } else {
        Err(RecycleError::Multiple(failures.join("; ")))
    }
}

/// Moves `path` in one Recycle Bin operation, after the safety guard, and
/// leaves it untouched when that fails.
fn move_whole(
    path: &Path,
    guard_roots: &[&Path],
    recycler: &dyn Recycler,
) -> Result<(), RecycleError> {
    if !is_safe_path(path, guard_roots) {
        return if path_missing(path) {
            Ok(())
        } else {
            Err(RecycleError::UnsafePath(path.to_path_buf()))
        };
    }
    match recycler.recycle(&[path]) {
        Err(_) if path_missing(path) => Ok(()),
        moved => moved,
    }
}

/// Recycles each immediate child of `path` on its own, re-running the safety
/// guard for each and never submitting a link. Problems go to `failures`.
/// Returns `false` when the folder could not be listed at all, or when `stop`
/// was set before every child was tried.
fn recycle_children(
    path: &Path,
    guard_roots: &[&Path],
    recycler: &dyn Recycler,
    stop: &AtomicBool,
    failures: &mut Vec<String>,
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
        let child = match entry {
            Ok(entry) => entry.path(),
            Err(error) => {
                failures.push(format!("could not enumerate a child: {error}"));
                continue;
            }
        };
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
        if let Err(error) = recycler.recycle(&[&child])
            && !path_missing(&child)
        {
            failures.push(format!("{}: {error}", child.display()));
        }
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
    use std::collections::HashSet;
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
                return Err(RecycleError::ShellOperation(5));
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
        assert!(mock.call_sizes().is_empty());
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
            Err(RecycleError::ShellOperation(5))
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
            exclusions: vec![cache.join("KEEP")],
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
        let result = execute_plain(&plan, false, &[&root], &mock, |_| {});

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
        let result = execute_plain(&plan, true, &[&root], &mock, |_| {});
        assert!(mock.call_sizes().is_empty());
        assert!(inside.exists());
        assert!(result.dry_run);
        assert!(result.groups.is_empty());
    }
}
