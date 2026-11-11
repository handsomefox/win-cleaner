//! What a cleanup leaves alone, beyond the safety guard: paths the user
//! excluded, files newer or smaller than the user's limits, and targets whose
//! app is running.
//!
//! The scan uses these rules to estimate sizes. Cleanup applies them again to
//! the live filesystem as it runs, with the rules as they are then, so an
//! exclusion added after the scan still protects its path.

use std::fs;
use std::path::{Component, MAIN_SEPARATOR, Path, PathBuf, Prefix};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, SystemTime};

use serde::{Deserialize, Serialize};

use crate::safety::{is_link, is_reparse_point, normalized_key};

/// The user's cleanup rules. Every field defaults to "no rule".
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Rules {
    /// Files and folders never cleaned. A folder protects everything in it.
    pub exclusions: Vec<PathBuf>,
    /// Leave files modified fewer than this many days ago. Zero is no limit.
    pub min_age_days: u32,
    /// Leave files smaller than this many bytes. Zero is no limit.
    pub min_size_bytes: u64,
    /// Skip a target while a program it belongs to is running.
    pub skip_running_apps: bool,
}

impl Rules {
    /// Whether an age or size limit applies, which means cleaning file by
    /// file instead of whole folders.
    #[must_use]
    pub fn filters_files(&self) -> bool {
        self.min_age_days > 0 || self.min_size_bytes > 0
    }

    /// Whether `path` is an exclusion or lies inside one.
    #[must_use]
    pub fn is_excluded(&self, path: &Path) -> bool {
        let key = normalized_key(path);
        self.exclusions
            .iter()
            .any(|exclusion| is_within(&key, &normalized_key(exclusion)))
    }

    /// Whether an exclusion lies strictly inside the folder `path`, so the
    /// folder cannot be cleaned as a whole.
    #[must_use]
    pub fn has_exclusion_inside(&self, path: &Path) -> bool {
        let key = normalized_key(path);
        self.exclusions.iter().any(|exclusion| {
            let exclusion = normalized_key(exclusion);
            exclusion != key && is_within(&exclusion, &key)
        })
    }

    /// Whether a file with this metadata is old and large enough to clean.
    #[must_use]
    pub fn file_passes(&self, metadata: &fs::Metadata, now: SystemTime) -> bool {
        if metadata.len() < self.min_size_bytes {
            return false;
        }
        if self.min_age_days == 0 {
            return true;
        }
        let limit = Duration::from_secs(u64::from(self.min_age_days) * 24 * 60 * 60);
        match metadata.modified() {
            // A file dated in the future is new, as far as this rule goes.
            Ok(modified) => now.duration_since(modified).is_ok_and(|age| age >= limit),
            Err(_) => false,
        }
    }

    /// Drops exclusions that are not plain absolute paths, and repeats.
    ///
    /// Cleanup targets are plain drive paths, and exclusions are matched to
    /// them by text, so `\\?\C:\cache` is rewritten as `C:\cache`. Any other
    /// prefix, such as a network share or a device, is dropped: no target
    /// sits under one, so it could never protect anything.
    pub fn sanitize(&mut self) {
        let mut seen = std::collections::HashSet::new();
        self.exclusions = std::mem::take(&mut self.exclusions)
            .into_iter()
            .filter_map(plain_absolute)
            .filter(|path| seen.insert(normalized_key(path)))
            .collect();
    }
}

/// `path` as a plain absolute path, or `None` when it has no such form.
fn plain_absolute(path: PathBuf) -> Option<PathBuf> {
    if !path.is_absolute() {
        return None;
    }
    let mut components = path.components();
    let Some(Component::Prefix(prefix)) = components.next() else {
        return Some(path);
    };
    match prefix.kind() {
        Prefix::Disk(_) => Some(path),
        Prefix::VerbatimDisk(letter) => {
            let mut plain = PathBuf::from(format!("{}:{MAIN_SEPARATOR}", char::from(letter)));
            plain.extend(components.filter(|part| !matches!(part, Component::RootDir)));
            Some(plain)
        }
        _ => None,
    }
}

/// Whether the normalized `key` equals `ancestor` or lies inside it. Compares
/// whole components, so `c:\foo` is not inside `c:\fo`.
fn is_within(key: &str, ancestor: &str) -> bool {
    match key.strip_prefix(ancestor) {
        Some("") => true,
        Some(rest) => rest.starts_with(MAIN_SEPARATOR) || ancestor.ends_with(MAIN_SEPARATOR),
        None => false,
    }
}

/// One file a filtered cleanup may move: its path, size, and modification
/// time.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileEntry {
    pub path: PathBuf,
    pub bytes: u64,
    pub modified: Option<SystemTime>,
}

/// How [`plan_paths`] walks: whether it sizes what it visits, and where it
/// reports the folders it could not read.
#[derive(Debug, Default)]
pub struct Walk {
    /// Size each visited path. Off for a caller that only needs the paths.
    pub measure: bool,
    /// One line per folder that could not be listed, such as one that needs
    /// administrator rights. Its contents are neither visited nor counted.
    pub errors: Vec<String>,
}

impl Walk {
    #[must_use]
    pub fn measured() -> Self {
        Self {
            measure: true,
            errors: Vec::new(),
        }
    }

    fn size(&self, path: &Path) -> u64 {
        if self.measure {
            crate::scan::dir_size(path).unwrap_or(0)
        } else {
            0
        }
    }

    fn read_dir(&mut self, folder: &Path) -> Option<Vec<PathBuf>> {
        match fs::read_dir(folder) {
            Ok(entries) => {
                let mut children: Vec<PathBuf> =
                    entries.flatten().map(|entry| entry.path()).collect();
                children.sort();
                Some(children)
            }
            Err(error) => {
                if error.kind() != std::io::ErrorKind::NotFound {
                    self.errors
                        .push(format!("could not read {}: {error}", folder.display()));
                }
                None
            }
        }
    }
}

/// What cleaning `root` under `rules` would move, as a list of whole paths
/// in the order they would go.
///
/// - An excluded `root` yields nothing.
/// - With an age or size limit, every file that passes it and is not
///   excluded, one by one. Folders stay.
/// - With an exclusion inside, each child that holds none whole, and the
///   children that do, split the same way one level down.
/// - Otherwise `root` itself, or its children when `keep_folder` is set.
///
/// Links are never followed or listed. `visit` sees each path with its size
/// when `walk` measures; returning `false` stops the walk.
pub fn plan_paths(
    root: &Path,
    keep_folder: bool,
    rules: &Rules,
    now: SystemTime,
    walk: &mut Walk,
    visit: &mut dyn FnMut(&Path, u64) -> bool,
) {
    if rules.is_excluded(root) {
        return;
    }
    let Ok(metadata) = fs::symlink_metadata(root) else {
        return;
    };
    if rules.filters_files() {
        walk_files(root, &metadata, rules, now, walk, visit);
        return;
    }
    let real_dir = metadata.is_dir() && !is_link(&metadata) && !is_reparse_point(&metadata);
    if real_dir && (keep_folder || rules.has_exclusion_inside(root)) {
        split_folder(root, rules, walk, visit);
    } else {
        let size = walk.size(root);
        visit(root, size);
    }
}

/// Visits each child of `folder` whole, going one level deeper only where an
/// exclusion sits inside. Returns `false` once `visit` asks to stop.
fn split_folder(
    folder: &Path,
    rules: &Rules,
    walk: &mut Walk,
    visit: &mut dyn FnMut(&Path, u64) -> bool,
) -> bool {
    let Some(children) = walk.read_dir(folder) else {
        return true;
    };
    for child in children {
        if rules.is_excluded(&child) {
            continue;
        }
        let Ok(metadata) = fs::symlink_metadata(&child) else {
            continue;
        };
        if is_link(&metadata) || is_reparse_point(&metadata) {
            continue;
        }
        let keep_going = if metadata.is_dir() && rules.has_exclusion_inside(&child) {
            split_folder(&child, rules, walk, visit)
        } else {
            let size = walk.size(&child);
            visit(&child, size)
        };
        if !keep_going {
            return false;
        }
    }
    true
}

/// Visits every file under `root` that passes the age and size limits and is
/// not excluded. Returns `false` once `visit` asks to stop.
fn walk_files(
    root: &Path,
    metadata: &fs::Metadata,
    rules: &Rules,
    now: SystemTime,
    walk: &mut Walk,
    visit: &mut dyn FnMut(&Path, u64) -> bool,
) -> bool {
    if is_link(metadata) || is_reparse_point(metadata) {
        return true;
    }
    if metadata.is_file() {
        if rules.file_passes(metadata, now) {
            return visit(root, metadata.len());
        }
        return true;
    }
    if !metadata.is_dir() {
        return true;
    }
    let Some(children) = walk.read_dir(root) else {
        return true;
    };
    for child in children {
        if rules.is_excluded(&child) {
            continue;
        }
        let Ok(child_metadata) = fs::symlink_metadata(&child) else {
            continue;
        };
        if !walk_files(&child, &child_metadata, rules, now, walk, visit) {
            return false;
        }
    }
    true
}

/// Every file cleaning `root` under `rules` would move, with its size and
/// modification time, for showing before a cleanup. A whole folder in the
/// plan is listed file by file. Stops after `limit` files, or as soon as
/// `cancel` is set, such as when the list is closed.
#[must_use]
pub fn list_files(
    root: &Path,
    keep_folder: bool,
    rules: &Rules,
    now: SystemTime,
    limit: usize,
    cancel: &AtomicBool,
) -> Vec<FileEntry> {
    let mut files = Vec::new();
    plan_paths(
        root,
        keep_folder,
        rules,
        now,
        &mut Walk::default(),
        &mut |path, _| {
            collect_files(path, &mut files, limit, cancel);
            files.len() < limit && !cancel.load(Ordering::Relaxed)
        },
    );
    files
}

fn collect_files(path: &Path, files: &mut Vec<FileEntry>, limit: usize, cancel: &AtomicBool) {
    if files.len() >= limit || cancel.load(Ordering::Relaxed) {
        return;
    }
    let Ok(metadata) = fs::symlink_metadata(path) else {
        return;
    };
    if is_link(&metadata) || is_reparse_point(&metadata) {
        return;
    }
    if metadata.is_file() {
        files.push(FileEntry {
            path: path.to_path_buf(),
            bytes: metadata.len(),
            modified: metadata.modified().ok(),
        });
    } else if metadata.is_dir()
        && let Ok(entries) = fs::read_dir(path)
    {
        for entry in entries.flatten() {
            collect_files(&entry.path(), files, limit, cancel);
            if files.len() >= limit || cancel.load(Ordering::Relaxed) {
                return;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::{create_dir_all, write};

    fn rules(exclusions: &[&Path]) -> Rules {
        Rules {
            exclusions: exclusions.iter().map(|path| path.to_path_buf()).collect(),
            ..Rules::default()
        }
    }

    fn planned(root: &Path, keep_folder: bool, rules: &Rules) -> Vec<(PathBuf, u64)> {
        let mut out = Vec::new();
        plan_paths(
            root,
            keep_folder,
            rules,
            SystemTime::now(),
            &mut Walk::measured(),
            &mut |path, size| {
                out.push((path.to_path_buf(), size));
                true
            },
        );
        out
    }

    #[test]
    fn exclusions_match_whole_components_without_regard_to_case() {
        let rules = rules(&[Path::new("/data/Cache/Keep")]);
        assert!(rules.is_excluded(Path::new("/data/cache/keep")));
        assert!(rules.is_excluded(Path::new("/data/cache/keep/deep/file")));
        assert!(!rules.is_excluded(Path::new("/data/cache/keeper")));
        assert!(!rules.is_excluded(Path::new("/data/cache")));
        assert!(rules.has_exclusion_inside(Path::new("/data/cache")));
        assert!(rules.has_exclusion_inside(Path::new("/data")));
        assert!(!rules.has_exclusion_inside(Path::new("/data/cache/keep")));
        assert!(!rules.has_exclusion_inside(Path::new("/data/cach")));
    }

    #[test]
    fn a_folder_without_rules_goes_whole() {
        let dir = tempfile::tempdir().unwrap();
        let cache = dir.path().join("cache");
        create_dir_all(cache.join("sub")).unwrap();
        write(cache.join("a"), [0; 10]).unwrap();
        write(cache.join("sub/b"), [0; 20]).unwrap();
        assert_eq!(
            planned(&cache, false, &Rules::default()),
            vec![(cache.clone(), 30)]
        );
        let children = planned(&cache, true, &Rules::default());
        assert_eq!(
            children,
            vec![(cache.join("a"), 10), (cache.join("sub"), 20)],
            "a kept folder lists its children"
        );
    }

    #[test]
    fn an_exclusion_inside_splits_the_folder_around_it() {
        let dir = tempfile::tempdir().unwrap();
        let cache = dir.path().join("cache");
        for (path, size) in [
            ("a", 10),
            ("sub/keep/k", 40),
            ("sub/drop", 5),
            ("other/x", 7),
        ] {
            let path = cache.join(path);
            create_dir_all(path.parent().unwrap()).unwrap();
            write(&path, vec![0; size]).unwrap();
        }
        let rules = rules(&[&cache.join("sub/keep")]);
        assert_eq!(
            planned(&cache, false, &rules),
            vec![
                (cache.join("a"), 10),
                (cache.join("other"), 7),
                (cache.join("sub/drop"), 5),
            ],
            "the excluded folder and the folders holding it stay"
        );
        assert!(planned(&cache.join("sub/keep"), false, &rules).is_empty());
        assert!(planned(&cache.join("sub/keep/k"), false, &rules).is_empty());
    }

    #[test]
    fn size_and_age_limits_clean_file_by_file() {
        let dir = tempfile::tempdir().unwrap();
        let cache = dir.path().join("cache");
        create_dir_all(cache.join("sub")).unwrap();
        write(cache.join("small"), [0; 10]).unwrap();
        write(cache.join("sub/big"), [0; 5000]).unwrap();
        let by_size = Rules {
            min_size_bytes: 1000,
            ..Rules::default()
        };
        assert_eq!(
            planned(&cache, false, &by_size),
            vec![(cache.join("sub/big"), 5000)]
        );

        let by_age = Rules {
            min_age_days: 1,
            ..Rules::default()
        };
        assert!(
            planned(&cache, false, &by_age).is_empty(),
            "files written just now are too new"
        );
        let later = SystemTime::now() + Duration::from_hours(72);
        let mut seen = Vec::new();
        plan_paths(
            &cache,
            false,
            &by_age,
            later,
            &mut Walk::default(),
            &mut |path, _| {
                seen.push(path.to_path_buf());
                true
            },
        );
        assert_eq!(seen, vec![cache.join("small"), cache.join("sub/big")]);
    }

    #[cfg(unix)]
    #[test]
    fn links_are_never_listed_or_followed() {
        let dir = tempfile::tempdir().unwrap();
        let outside = dir.path().join("outside");
        create_dir_all(&outside).unwrap();
        write(outside.join("precious"), [0; 100]).unwrap();
        let cache = dir.path().join("cache");
        create_dir_all(&cache).unwrap();
        std::os::unix::fs::symlink(&outside, cache.join("link")).unwrap();
        write(cache.join("ok"), [0; 1]).unwrap();
        let limited = Rules {
            min_size_bytes: 1,
            ..Rules::default()
        };
        assert_eq!(
            planned(&cache, false, &limited),
            vec![(cache.join("ok"), 1)]
        );
        assert_eq!(
            planned(&cache, true, &Rules::default()),
            vec![(cache.join("ok"), 1)]
        );
        assert!(
            list_files(
                &cache,
                false,
                &limited,
                SystemTime::now(),
                10,
                &AtomicBool::new(false)
            )
            .iter()
            .all(|file| !file.path.starts_with(&outside))
        );
    }

    #[cfg(unix)]
    #[test]
    fn an_unreadable_folder_is_reported_not_skipped_silently() {
        use std::os::unix::fs::PermissionsExt as _;
        let dir = tempfile::tempdir().unwrap();
        let locked = dir.path().join("Prefetch");
        create_dir_all(&locked).unwrap();
        write(locked.join("a.pf"), [0; 5]).unwrap();
        fs::set_permissions(&locked, fs::Permissions::from_mode(0o000)).unwrap();
        let readable = fs::read_dir(&locked).is_ok();
        let mut walk = Walk::measured();
        plan_paths(
            &locked,
            true,
            &Rules::default(),
            SystemTime::now(),
            &mut walk,
            &mut |_, _| true,
        );
        fs::set_permissions(&locked, fs::Permissions::from_mode(0o755)).unwrap();
        if !readable {
            assert_eq!(walk.errors.len(), 1, "{:?}", walk.errors);
            assert!(walk.errors[0].contains("Prefetch"));
        }
    }

    #[test]
    fn listing_stops_at_the_limit_and_expands_folders() {
        let dir = tempfile::tempdir().unwrap();
        let cache = dir.path().join("cache");
        create_dir_all(cache.join("a/b")).unwrap();
        for index in 0..5 {
            write(cache.join(format!("a/b/f{index}")), [0; 3]).unwrap();
        }
        let files = list_files(
            &cache,
            false,
            &Rules::default(),
            SystemTime::now(),
            3,
            &AtomicBool::new(false),
        );
        assert_eq!(files.len(), 3);
        let cancelled = list_files(
            &cache,
            false,
            &Rules::default(),
            SystemTime::now(),
            3,
            &AtomicBool::new(true),
        );
        assert!(cancelled.is_empty(), "a closed list stops walking");
        assert_eq!(files.len(), 3);
        assert!(
            files
                .iter()
                .all(|file| file.bytes == 3 && file.modified.is_some())
        );
    }

    #[cfg(windows)]
    #[test]
    fn sanitize_rewrites_verbatim_drive_paths_and_drops_other_prefixes() {
        let mut rules = Rules {
            exclusions: vec![
                PathBuf::from(r"\\?\C:\Users\Ed\AppData\Local\cache\keep.bin"),
                PathBuf::from(r"\\server\share\keep"),
                PathBuf::from(r"\\?\UNC\server\share\keep"),
                PathBuf::from(r"\\.\C:\keep"),
            ],
            ..Rules::default()
        };
        rules.sanitize();
        assert_eq!(
            rules.exclusions,
            vec![PathBuf::from(r"C:\Users\Ed\AppData\Local\cache\keep.bin")]
        );
        let cache = Path::new(r"C:\Users\Ed\AppData\Local\cache");
        assert!(rules.has_exclusion_inside(cache));
        assert!(rules.is_excluded(&cache.join("keep.bin")));
    }

    #[test]
    fn sanitize_drops_relative_and_repeated_exclusions() {
        let base = std::env::temp_dir();
        let mut rules = Rules {
            exclusions: vec![
                PathBuf::from("relative/path"),
                base.join("Keep"),
                base.join("keep"),
            ],
            ..Rules::default()
        };
        rules.sanitize();
        assert_eq!(rules.exclusions, vec![base.join("Keep")]);
    }
}
