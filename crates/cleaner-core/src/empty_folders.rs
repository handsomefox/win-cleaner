//! Opt-in detection of empty top-level folders under the four `AppData` /
//! `ProgramData` roots. A folder qualifies only when its entire subtree
//! contains zero files: reparse points and symlinks count as content, and a
//! directory that cannot be read is treated as non-empty so a read error can
//! never lead to recycling.

use std::fs;
use std::path::{Path, PathBuf};

use crate::plan::{Group, Target};
use crate::roots::Roots;
use crate::rules::trimmed_by_windows;
use crate::safety::{is_reparse_point, is_safe_path};

/// App name used for the empty-folder removal groups; the GUI keys its
/// category mapping off this value.
pub const EMPTY_FOLDERS_APP: &str = "Empty folders";

/// The roots swept for empty top-level folders, paired with their UI labels.
/// Only immediate children of these roots are ever candidates for removal.
fn empty_folder_roots(roots: &Roots) -> Vec<(String, PathBuf)> {
    let mut out = Vec::with_capacity(4);
    let mut add = |label: &str, path: Option<PathBuf>| {
        if let Some(path) = path {
            out.push((label.to_owned(), path));
        }
    };
    add(r"AppData\Local", roots.local_app_data.clone());
    add(r"AppData\LocalLow", roots.local_low());
    add(r"AppData\Roaming", roots.roaming_app_data.clone());
    add("ProgramData", roots.program_data.clone());
    out
}

/// Scans the direct children of each empty-folder root and returns one
/// [`Group`] per root that has at least one qualifying folder. Groups carry
/// `bytes: 0` and are never pre-selected (opt-in only).
#[must_use]
pub fn build_empty_folder_groups(roots: &Roots) -> Vec<Group> {
    empty_folder_groups(empty_folder_roots(roots), &roots.guard_roots())
}

/// Builds the groups for `sweep_roots`, offering only candidates that pass the
/// safe-root guard, the same check the catalog paths get at scan time.
fn empty_folder_groups(sweep_roots: Vec<(String, PathBuf)>, guard_roots: &[&Path]) -> Vec<Group> {
    let mut groups = Vec::new();
    for (label, root) in sweep_roots {
        let Ok(entries) = fs::read_dir(&root) else {
            continue;
        };
        let mut candidates: Vec<PathBuf> = Vec::new();
        for entry in entries.flatten() {
            // Windows would look `name.` up as `name`, another folder.
            if trimmed_by_windows(&entry.file_name()) || !entry_is_plain_dir(&entry) {
                continue;
            }
            let child = root.join(entry.file_name());
            if let Some(empty_tree) = empty_subtree(&child) {
                candidates.extend(
                    empty_tree
                        .into_iter()
                        .filter(|path| is_safe_path(path, guard_roots)),
                );
            }
        }
        if candidates.is_empty() {
            continue;
        }
        // Keep each subtree in postorder so execution removes children before
        // their parents, using non-recursive remove_dir for every path.
        groups.push(Group {
            targets: candidates
                .into_iter()
                .map(|path| Target::whole(path, 0))
                .collect(),
            ..Group::new(EMPTY_FOLDERS_APP, &label)
        });
    }
    groups
}

/// Reports whether the entry is a real directory that is not a symlink or
/// reparse point (junction, mount point, cloud placeholder).
fn entry_is_plain_dir(entry: &fs::DirEntry) -> bool {
    let Ok(file_type) = entry.file_type() else {
        return false;
    };
    if file_type.is_symlink() || !file_type.is_dir() {
        return false;
    }
    // The cheap file_type check can miss reparse points on Windows; confirm
    // via metadata attributes.
    match entry.metadata() {
        Ok(metadata) => !is_reparse_point(&metadata),
        Err(_) => false,
    }
}

/// Reports whether `dir`'s entire subtree contains zero files, and if so
/// returns every folder in it, `dir` last, in postorder: each folder after
/// the folders inside it, siblings sorted. All entries of a directory are
/// checked for non-directories before any subdirectory is descended into, so
/// the first file found anywhere bails out early. A name Windows would trim
/// counts as content, since removing it by name would reach another folder.
///
/// The walk keeps its pending folders on the heap, so a deeply nested tree
/// cannot overflow the worker thread's stack.
fn empty_subtree(dir: &Path) -> Option<Vec<PathBuf>> {
    /// A folder whose subfolders are still being visited.
    struct Open {
        path: PathBuf,
        subdirs: std::vec::IntoIter<PathBuf>,
    }
    let mut paths = Vec::new();
    let mut pending = vec![Open {
        subdirs: plain_subdirs(dir)?.into_iter(),
        path: dir.to_path_buf(),
    }];
    while let Some(open) = pending.last_mut() {
        if let Some(sub) = open.subdirs.next() {
            pending.push(Open {
                subdirs: plain_subdirs(&sub)?.into_iter(),
                path: sub,
            });
        } else if let Some(done) = pending.pop() {
            paths.push(done.path);
        }
    }
    Some(paths)
}

/// The subfolders of `dir`, sorted, or `None` when it holds anything else or
/// cannot be read.
fn plain_subdirs(dir: &Path) -> Option<Vec<PathBuf>> {
    let entries = fs::read_dir(dir).ok()?;
    let mut subdirs = Vec::new();
    for entry in entries {
        let entry = entry.ok()?;
        if trimmed_by_windows(&entry.file_name()) || !entry_is_plain_dir(&entry) {
            return None;
        }
        subdirs.push(dir.join(entry.file_name()));
    }
    subdirs.sort();
    Some(subdirs)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn paths(group: &Group) -> Vec<PathBuf> {
        group
            .targets
            .iter()
            .map(|target| target.path.clone())
            .collect()
    }
    use std::fs::{File, create_dir_all};

    fn subtree_has_no_files(dir: &Path) -> bool {
        empty_subtree(dir).is_some()
    }

    #[test]
    fn nested_empty_tree_is_removed_without_recycling_new_content() {
        use crate::{Options, Plan, RecycleError, Recycler, execute_with_result};
        struct NoRecycler;
        impl Recycler for NoRecycler {
            fn recycle(&self, _paths: &[&Path]) -> Result<(), RecycleError> {
                panic!("empty trees must only use non-recursive removal");
            }
        }
        for changed in [false, true] {
            let dir = tempfile::tempdir().unwrap();
            let local = dir.path().join("Local");
            let parent = local.join("EmptyApp");
            let leaf = parent.join("a/b");
            create_dir_all(&leaf).unwrap();
            let roots = Roots {
                local_app_data: Some(local),
                ..Roots::default()
            };
            let mut plan = Plan {
                groups: build_empty_folder_groups(&roots),
                ..Plan::default()
            };
            for group in &mut plan.groups {
                group.on = true;
            }
            plan.recompute_totals();
            if changed {
                fs::write(leaf.join("new.txt"), b"keep me").unwrap();
            }
            let stop = std::sync::atomic::AtomicBool::new(false);
            let rules = crate::rules::Rules::default();
            let guard_roots = roots.guard_roots();
            let run = crate::execute::Run {
                options: Options { dry_run: false },
                guard_roots: &guard_roots,
                rules: &rules,
                stop: &stop,
                is_running: &|_| false,
            };
            let result = execute_with_result(&plan, &run, &NoRecycler, |_| {});
            if changed {
                assert!(result.error_count > 0);
                assert_eq!(fs::read(leaf.join("new.txt")).unwrap(), b"keep me");
            } else {
                assert_eq!(result.error_count, 0);
                assert!(!parent.exists());
            }
        }
    }

    #[test]
    fn empty_and_nested_empty_dirs_qualify() {
        let dir = tempfile::tempdir().unwrap();
        create_dir_all(dir.path().join("empty")).unwrap();
        create_dir_all(dir.path().join("nested/a/b/c")).unwrap();
        assert!(subtree_has_no_files(&dir.path().join("empty")));
        assert!(subtree_has_no_files(&dir.path().join("nested")));
    }

    #[test]
    fn any_file_disqualifies() {
        let dir = tempfile::tempdir().unwrap();
        create_dir_all(dir.path().join("d/a/b")).unwrap();
        File::create(dir.path().join("d/a/b/file.txt")).unwrap();
        assert!(!subtree_has_no_files(&dir.path().join("d")));
    }

    #[cfg(unix)]
    #[test]
    fn symlinks_count_as_content() {
        let dir = tempfile::tempdir().unwrap();
        create_dir_all(dir.path().join("target")).unwrap();
        create_dir_all(dir.path().join("d")).unwrap();
        std::os::unix::fs::symlink(dir.path().join("target"), dir.path().join("d/link")).unwrap();
        assert!(!subtree_has_no_files(&dir.path().join("d")));
    }

    #[test]
    fn a_name_windows_would_trim_is_content() {
        let dir = tempfile::tempdir().unwrap();
        create_dir_all(dir.path().join("d/a")).unwrap();
        // Removing `d\a.` by name would remove `d\a`.
        create_dir_all(crate::rules::verbatim_for_test(
            &dir.path().join("d").join("a."),
        ))
        .unwrap();
        assert!(!subtree_has_no_files(&dir.path().join("d")));

        let local = dir.path().join("Local");
        create_dir_all(local.join("EmptyApp")).unwrap();
        create_dir_all(crate::rules::verbatim_for_test(&local.join("EmptyApp."))).unwrap();
        let groups = empty_folder_groups(vec![("Local".to_owned(), local.clone())], &[&local]);
        assert_eq!(paths(&groups[0]), vec![local.join("EmptyApp")]);
    }

    #[test]
    fn a_deeply_nested_empty_tree_does_not_overflow_the_stack() {
        // 1500 levels of `d/` fit in a Linux path. On a 256 KiB stack, a walk
        // that recursed once per level would abort.
        const DEPTH: usize = 1500;
        let dir = tempfile::tempdir().unwrap();
        let top = dir.path().join("top");
        let mut deepest = top.clone();
        for _ in 0..DEPTH {
            deepest.push("d");
        }
        create_dir_all(&deepest).unwrap();
        let walked = std::thread::Builder::new()
            .stack_size(256 * 1024)
            .spawn({
                let top = top.clone();
                move || empty_subtree(&top)
            })
            .unwrap()
            .join()
            .unwrap()
            .unwrap();
        assert_eq!(walked.len(), DEPTH + 1);
        assert_eq!(
            walked.first(),
            Some(&deepest),
            "the deepest folder goes first"
        );
        assert_eq!(walked.last(), Some(&top), "the top folder goes last");
    }

    #[test]
    fn postorder_visits_siblings_sorted_and_children_first() {
        let dir = tempfile::tempdir().unwrap();
        let top = dir.path().join("top");
        create_dir_all(top.join("b/y")).unwrap();
        create_dir_all(top.join("a/x")).unwrap();
        create_dir_all(top.join("a/w")).unwrap();
        assert_eq!(
            empty_subtree(&top).unwrap(),
            vec![
                top.join("a/w"),
                top.join("a/x"),
                top.join("a"),
                top.join("b/y"),
                top.join("b"),
                top.clone(),
            ]
        );
    }

    #[test]
    fn missing_dir_is_not_empty() {
        let dir = tempfile::tempdir().unwrap();
        assert!(!subtree_has_no_files(&dir.path().join("missing")));
    }

    #[cfg(unix)]
    #[test]
    fn unreadable_dir_is_not_empty() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let locked = dir.path().join("locked");
        create_dir_all(&locked).unwrap();
        fs::set_permissions(&locked, fs::Permissions::from_mode(0o000)).unwrap();
        let result = subtree_has_no_files(&locked);
        fs::set_permissions(&locked, fs::Permissions::from_mode(0o755)).unwrap();
        assert!(!result);
    }

    #[test]
    fn candidates_outside_the_safe_roots_are_not_offered() {
        let dir = tempfile::tempdir().unwrap();
        let safe = dir.path().join("Local");
        let outside = dir.path().join("Elsewhere");
        create_dir_all(safe.join("EmptyApp")).unwrap();
        create_dir_all(outside.join("EmptyApp")).unwrap();

        let groups = empty_folder_groups(
            vec![
                ("Local".to_owned(), safe.clone()),
                ("Elsewhere".to_owned(), outside),
            ],
            &[&safe],
        );
        assert_eq!(groups.len(), 1, "the outside root offers nothing");
        assert_eq!(paths(&groups[0]), vec![safe.join("EmptyApp")]);
    }

    #[test]
    fn groups_are_opt_in_per_root() {
        let dir = tempfile::tempdir().unwrap();
        let local = dir.path().join("Local");
        let roaming = dir.path().join("Roaming");
        create_dir_all(local.join("EmptyApp")).unwrap();
        create_dir_all(local.join("FullApp")).unwrap();
        File::create(local.join("FullApp/data.db")).unwrap();
        create_dir_all(roaming.join("OnlyNested/deep")).unwrap();

        let roots = Roots {
            local_app_data: Some(local.clone()),
            roaming_app_data: Some(roaming.clone()),
            program_data: Some(dir.path().join("ProgramData")), // missing on disk
            user_profile: Some(dir.path().join("Profile")),
            ..Roots::default()
        };

        let groups = build_empty_folder_groups(&roots);
        assert_eq!(groups.len(), 2);

        let local_group = &groups[0];
        assert_eq!(local_group.app, EMPTY_FOLDERS_APP);
        assert_eq!(local_group.label, r"AppData\Local");
        assert_eq!(paths(local_group), vec![local.join("EmptyApp")]);
        assert_eq!(local_group.bytes, 0);
        assert!(!local_group.on);

        let roaming_group = &groups[1];
        assert_eq!(roaming_group.label, r"AppData\Roaming");
        assert_eq!(
            paths(roaming_group),
            vec![roaming.join("OnlyNested/deep"), roaming.join("OnlyNested")]
        );
    }
}
