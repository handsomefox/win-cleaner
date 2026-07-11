//! Opt-in detection of empty top-level folders under the four `AppData` /
//! `ProgramData` roots. A folder qualifies only when its entire subtree
//! contains zero files: reparse points and symlinks count as content, and a
//! directory that cannot be read is treated as non-empty so a read error can
//! never lead to recycling.

use std::fs;
use std::path::{Path, PathBuf};

use crate::plan::Group;
use crate::roots::Roots;
use crate::safety::is_reparse_point;

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
    let mut groups = Vec::new();
    for (label, root) in empty_folder_roots(roots) {
        let Ok(entries) = fs::read_dir(&root) else {
            continue;
        };
        let mut candidates: Vec<PathBuf> = Vec::new();
        for entry in entries.flatten() {
            if !entry_is_plain_dir(&entry) {
                continue;
            }
            let child = root.join(entry.file_name());
            if subtree_has_no_files(&child) {
                candidates.push(child);
            }
        }
        if candidates.is_empty() {
            continue;
        }
        candidates.sort();
        groups.push(Group {
            app: EMPTY_FOLDERS_APP.to_owned(),
            label,
            paths: candidates,
            errs: Vec::new(),
            bytes: 0,
            on: false,
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

/// Reports whether `dir`'s entire subtree contains zero files. All entries of
/// a directory are checked for non-directories before any subdirectory is
/// descended into, so the first file found anywhere bails out early.
fn subtree_has_no_files(dir: &Path) -> bool {
    let Ok(entries) = fs::read_dir(dir) else {
        return false;
    };

    let mut subdirs = Vec::new();
    for entry in entries {
        let Ok(entry) = entry else {
            return false;
        };
        if !entry_is_plain_dir(&entry) {
            return false;
        }
        subdirs.push(dir.join(entry.file_name()));
    }
    subdirs.iter().all(|sub| subtree_has_no_files(sub))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::{File, create_dir_all};

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
        assert_eq!(local_group.paths, vec![local.join("EmptyApp")]);
        assert_eq!(local_group.bytes, 0);
        assert!(!local_group.on);

        let roaming_group = &groups[1];
        assert_eq!(roaming_group.label, r"AppData\Roaming");
        assert_eq!(roaming_group.paths, vec![roaming.join("OnlyNested")]);
    }
}
