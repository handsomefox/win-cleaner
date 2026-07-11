use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use crate::catalog::Registry;
use crate::empty_folders::build_empty_folder_groups;
use crate::globs::expand;
use crate::plan::{Group, Phase, Plan, ProgressUpdate};
use crate::roots::Roots;
use crate::safety::{is_safe_path, normalized_key};

/// Builds the cleanup plan: resolves catalog paths and globs, applies the
/// safety guard, estimates reclaimable sizes, and appends the opt-in
/// empty-folder groups. `progress` is invoked once per catalog item.
pub fn build_plan(
    registry: &Registry,
    roots: &Roots,
    mut progress: impl FnMut(ProgressUpdate),
) -> Plan {
    let guard_roots = roots.guard_roots();
    let total_items = registry.items.len();

    let mut groups: Vec<Group> = Vec::with_capacity(total_items);
    for (index, item) in registry.items.iter().enumerate() {
        let mut resolved: Vec<PathBuf> = item
            .paths
            .iter()
            .filter(|path| !path.as_os_str().is_empty())
            .cloned()
            .collect();
        for pattern in &item.globs {
            resolved.extend(expand(pattern));
        }
        let resolved = unique_paths(resolved);

        let mut bytes = 0u64;
        let mut errs = Vec::new();
        for path in &resolved {
            if !is_safe_path(path, &guard_roots) {
                errs.push(format!("skipping unsafe path: {}", path.display()));
                continue;
            }
            match dir_size(path) {
                Ok(size) => bytes += size,
                Err(err) => errs.push(format!("{}: {err}", path.display())),
            }
        }

        groups.push(Group {
            app: item.app.clone(),
            label: item.label.clone(),
            paths: resolved,
            // Never pre-select items with nothing to clean.
            on: item.default_on && bytes > 0,
            bytes,
            errs,
        });

        progress(ProgressUpdate {
            phase: Phase::Scan,
            current: index + 1,
            total: total_items,
            message: format!("{} - {}", item.app, item.label),
        });
    }

    // Opt-in empty-folder removal groups (one per scanned root).
    groups.extend(build_empty_folder_groups(roots));

    groups.sort_by(|a, b| a.app.cmp(&b.app).then_with(|| a.label.cmp(&b.label)));

    let mut plan = Plan {
        groups,
        ..Plan::default()
    };
    plan.recompute_totals();
    plan
}

/// De-duplicates paths case-insensitively on the cleaned path key, keeping
/// first occurrences in order.
fn unique_paths(paths: Vec<PathBuf>) -> Vec<PathBuf> {
    let mut seen = HashSet::new();
    let mut out = Vec::with_capacity(paths.len());
    for path in paths {
        if seen.insert(normalized_key(&path)) {
            out.push(path);
        }
    }
    out
}

/// Total byte size of a path. Returns `Ok(0)` for non-existent paths to avoid
/// noisy errors. Symlinks and reparse points are never followed or sized.
fn dir_size(path: &Path) -> std::io::Result<u64> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(0),
        Err(err) => return Err(err),
    };
    if metadata.file_type().is_symlink() || is_reparse_point(&metadata) {
        return Ok(0);
    }
    if !metadata.is_dir() {
        return Ok(metadata.len());
    }

    let mut total = 0u64;
    for entry in walkdir::WalkDir::new(path)
        .follow_links(false)
        .into_iter()
        .filter_map(Result::ok)
    {
        let file_type = entry.file_type();
        if file_type.is_symlink() || !file_type.is_file() {
            continue;
        }
        if let Ok(metadata) = entry.metadata()
            && !is_reparse_point(&metadata)
        {
            total += metadata.len();
        }
    }
    Ok(total)
}

/// Reports whether metadata describes a Windows reparse point (junction,
/// mount point, cloud placeholder). Always false elsewhere, where plain
/// symlink checks suffice.
pub(crate) fn is_reparse_point(metadata: &fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0400;
        metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
    }
    #[cfg(not(windows))]
    {
        let _ = metadata;
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::Item;
    use std::fs::{File, create_dir_all};
    use std::io::Write as _;

    fn write_file(path: &Path, len: usize) {
        create_dir_all(path.parent().unwrap()).unwrap();
        let mut file = File::create(path).unwrap();
        file.write_all(&vec![0u8; len]).unwrap();
    }

    fn test_roots(base: &Path) -> Roots {
        Roots {
            local_app_data: Some(base.join("Local")),
            roaming_app_data: Some(base.join("Roaming")),
            program_data: Some(base.join("ProgramData")),
            user_profile: Some(base.join("Profile")),
            ..Roots::default()
        }
    }

    #[test]
    fn unique_paths_is_case_insensitive_keeping_first() {
        let paths = vec![
            PathBuf::from("/a/B/c"),
            PathBuf::from("/a/b/C"),
            PathBuf::from("/a/b/d"),
        ];
        let unique = unique_paths(paths);
        assert_eq!(
            unique,
            vec![PathBuf::from("/a/B/c"), PathBuf::from("/a/b/d")]
        );
    }

    #[test]
    fn dir_size_sums_files_and_skips_symlinks() {
        let dir = tempfile::tempdir().unwrap();
        write_file(&dir.path().join("cache/a.bin"), 100);
        write_file(&dir.path().join("cache/sub/b.bin"), 50);
        write_file(&dir.path().join("outside.bin"), 9999);

        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(
                dir.path().join("outside.bin"),
                dir.path().join("cache/link.bin"),
            )
            .unwrap();
            std::os::unix::fs::symlink(dir.path(), dir.path().join("cache/dirlink")).unwrap();
        }

        assert_eq!(dir_size(&dir.path().join("cache")).unwrap(), 150);
        assert_eq!(dir_size(&dir.path().join("missing")).unwrap(), 0);
        assert_eq!(dir_size(&dir.path().join("outside.bin")).unwrap(), 9999);
    }

    #[test]
    fn build_plan_expands_globs_and_respects_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let roots = test_roots(dir.path());
        let local = roots.local_app_data.clone().unwrap();

        write_file(&local.join("app/Profile1/Cache/f_0001"), 1000);
        write_file(&local.join("npm-cache/pkg.tgz"), 500);
        write_file(&local.join("shaders/blob.bin"), 100);

        let registry = Registry {
            items: vec![
                Item::new("App", "profile caches", true)
                    .globs([local.join("app").join("*").join("Cache")]),
                Item::new("npm", "package cache", true).paths([local.join("npm-cache")]),
                Item::new("Missing", "empty target", true).paths([local.join("nothing-here")]),
                Item::new("Shaders", "opt-in cache", false).paths([local.join("shaders")]),
            ],
        };
        let mut updates = 0;
        let plan = build_plan(&registry, &roots, |_| updates += 1);
        assert_eq!(updates, registry.items.len());

        let app = plan.groups.iter().find(|group| group.app == "App").unwrap();
        assert_eq!(app.bytes, 1000);
        assert!(app.on, "non-empty default-on group is pre-selected");

        let npm = plan.groups.iter().find(|group| group.app == "npm").unwrap();
        assert_eq!(npm.bytes, 500);
        assert!(npm.on);

        // Empty default-on groups are never pre-selected.
        let missing = plan
            .groups
            .iter()
            .find(|group| group.app == "Missing")
            .unwrap();
        assert_eq!(missing.bytes, 0);
        assert!(!missing.on);

        // Opt-in groups stay off even with content.
        let shaders = plan
            .groups
            .iter()
            .find(|group| group.app == "Shaders")
            .unwrap();
        assert_eq!(shaders.bytes, 100);
        assert!(!shaders.on);

        assert_eq!(plan.selected, 2);
        assert_eq!(plan.total_bytes, 1500);

        // Groups are sorted by app then label.
        let names: Vec<&str> = plan.groups.iter().map(|g| g.app.as_str()).collect();
        let mut sorted = names.clone();
        sorted.sort_unstable();
        assert_eq!(names, sorted);
    }

    #[test]
    fn build_plan_flags_unsafe_paths() {
        let dir = tempfile::tempdir().unwrap();
        let roots = test_roots(dir.path());
        let registry = Registry {
            items: vec![
                Item::new("Evil", "outside", true).paths([PathBuf::from("/etc/passwd-like")]),
            ],
        };

        let plan = build_plan(&registry, &roots, |_| {});
        let evil = plan.groups.iter().find(|g| g.app == "Evil").unwrap();
        assert_eq!(evil.errs.len(), 1);
        assert!(evil.errs[0].contains("unsafe"));
        assert_eq!(evil.bytes, 0);
    }
}
