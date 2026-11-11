use std::collections::HashSet;
use std::fs;
use std::path::Path;

use crate::catalog::Registry;
use crate::empty_folders::build_empty_folder_groups;
use crate::globs::expand;
use std::time::SystemTime;

use crate::plan::{Group, Plan, ProgressUpdate, Target};
use crate::roots::Roots;
use crate::rules::{Rules, Walk, plan_paths};
use crate::safety::{is_link, is_reparse_point, is_safe_path, normalized_key};
use crate::versions::superseded;

/// Builds the cleanup plan: resolves catalog paths and globs, applies the
/// safety guard and the user's rules, estimates reclaimable sizes, and
/// appends the opt-in empty-folder groups. `progress` is invoked once per
/// catalog item, and `is_running` tells whether a program is running.
pub fn build_plan(
    registry: &Registry,
    roots: &Roots,
    rules: &Rules,
    is_running: &dyn Fn(&str) -> bool,
    mut progress: impl FnMut(ProgressUpdate),
) -> Plan {
    let guard_roots = roots.guard_roots();
    let total_items = registry.items.len();
    let now = SystemTime::now();

    let mut groups: Vec<Group> = Vec::with_capacity(total_items);
    for (index, item) in registry.items.iter().enumerate() {
        let mut errs = Vec::new();
        let mut resolved: Vec<Target> = item
            .paths
            .iter()
            .filter(|path| !path.as_os_str().is_empty())
            .map(|path| Target::whole(path.clone(), 0))
            .collect();
        for pattern in &item.globs {
            if is_safe_path(pattern, &guard_roots) {
                resolved.extend(
                    expand(pattern)
                        .into_iter()
                        .map(|path| Target::whole(path, 0)),
                );
            } else {
                errs.push(format!("skipping unsafe glob: {}", pattern.display()));
            }
        }
        resolved.extend(
            item.contents
                .iter()
                .filter(|path| !path.as_os_str().is_empty())
                .map(|path| Target {
                    keep_folder: true,
                    ..Target::whole(path.clone(), 0)
                }),
        );
        for rule in &item.versioned {
            if is_safe_path(&rule.dir, &guard_roots) {
                resolved.extend(superseded(rule).into_iter().map(|path| Target {
                    version_rule: Some(rule.clone()),
                    ..Target::whole(path, 0)
                }));
            } else {
                errs.push(format!(
                    "skipping unsafe version folder: {}",
                    rule.dir.display()
                ));
            }
        }
        let mut resolved = unique_targets(resolved);

        let mut bytes = 0u64;
        for target in &mut resolved {
            if !is_safe_path(&target.path, &guard_roots) {
                errs.push(format!("skipping unsafe path: {}", target.path.display()));
                continue;
            }
            target.bytes = estimate(target, rules, now, &mut errs);
            bytes += target.bytes;
        }
        // A target the rules leave entirely alone is not offered at all.
        resolved.retain(|target| {
            !rules.is_excluded(&target.path)
                && (target.version_rule.is_none() || !rules.has_exclusion_inside(&target.path))
        });

        let running = if rules.skip_running_apps {
            item.processes.iter().find(|name| is_running(name)).cloned()
        } else {
            None
        };
        groups.push(Group {
            targets: resolved,
            // Never pre-select items with nothing to clean, or whose program
            // is running.
            on: item.default_on && bytes > 0 && running.is_none(),
            bytes,
            errs,
            processes: item.processes.clone(),
            admin: item.admin,
            running,
            ..Group::new(&item.app, &item.label)
        });

        progress(ProgressUpdate {
            current: index + 1,
            total: total_items,
            message: format!("{} - {}", item.app, item.label),
        });
    }

    // Opt-in empty-folder removal groups (one per scanned root).
    groups.extend(
        build_empty_folder_groups(roots)
            .into_iter()
            .map(|mut group| {
                // A folder holding an exclusion is not empty to the user either.
                group.targets.retain(|target| {
                    !rules.is_excluded(&target.path) && !rules.has_exclusion_inside(&target.path)
                });
                group
            }),
    );

    // Apps sort by name; within one, targets keep the catalog's order, which
    // puts a browser's channels as Stable, Beta, Dev, Canary.
    groups.sort_by(|a, b| a.app.cmp(&b.app));

    let mut plan = Plan {
        groups,
        ..Plan::default()
    };
    plan.recompute_totals();
    plan
}

/// The bytes cleaning `target` under `rules` would move, as cleanup will
/// plan it: a program version whole or not at all, anything else as
/// [`plan_paths`] splits it.
fn estimate(target: &Target, rules: &Rules, now: SystemTime, errs: &mut Vec<String>) -> u64 {
    if target.version_rule.is_some() {
        if rules.is_excluded(&target.path) || rules.has_exclusion_inside(&target.path) {
            return 0;
        }
        return match dir_size(&target.path) {
            Ok(size) => size,
            Err(err) => {
                errs.push(format!("{}: {err}", target.path.display()));
                0
            }
        };
    }
    if !target.keep_folder && !rules.filters_files() && !rules.has_exclusion_inside(&target.path) {
        if rules.is_excluded(&target.path) {
            return 0;
        }
        return match dir_size(&target.path) {
            Ok(size) => size,
            Err(err) => {
                errs.push(format!("{}: {err}", target.path.display()));
                0
            }
        };
    }
    let mut total = 0u64;
    let mut walk = Walk::measured();
    plan_paths(
        &target.path,
        target.keep_folder,
        rules,
        now,
        &mut walk,
        &mut |_, size| {
            total += size;
            true
        },
    );
    errs.extend(walk.errors);
    total
}

/// De-duplicates targets case-insensitively on the cleaned path key, keeping
/// first occurrences in order.
fn unique_targets(targets: Vec<Target>) -> Vec<Target> {
    let mut seen = HashSet::new();
    let mut out = Vec::with_capacity(targets.len());
    for target in targets {
        if seen.insert(normalized_key(&target.path)) {
            out.push(target);
        }
    }
    out
}

/// Total byte size of a path. Returns `Ok(0)` for non-existent paths to avoid
/// noisy errors. Symlinks and reparse points are never followed or sized.
pub(crate) fn dir_size(path: &Path) -> std::io::Result<u64> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(0),
        Err(err) => return Err(err),
    };
    if is_link(&metadata) {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::Item;
    use std::fs::{create_dir_all, write};
    use std::path::PathBuf;

    fn write_file(path: &Path, len: usize) {
        create_dir_all(path.parent().unwrap()).unwrap();
        write(path, vec![0u8; len]).unwrap();
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
    fn rules_shape_the_estimate_and_the_selection() {
        let dir = tempfile::tempdir().unwrap();
        let roots = test_roots(dir.path());
        let local = roots.local_app_data.clone().unwrap();
        write_file(&local.join("App/Cache/a.bin"), 100);
        write_file(&local.join("App/Cache/keep/b.bin"), 50);
        write_file(&local.join("Other/Cache/c.bin"), 10);
        let registry = Registry {
            items: vec![
                Item::new("App", "cache", true).paths([local.join("App/Cache")]),
                Item::new("Other", "cache", true)
                    .paths([local.join("Other/Cache")])
                    .processes(["Other.exe"]),
            ],
        };
        let rules = Rules {
            exclusions: vec![local.join("App/Cache/keep")],
            skip_running_apps: true,
            ..Rules::default()
        };
        let plan = build_plan(
            &registry,
            &roots,
            &rules,
            &|name| name == "Other.exe",
            |_| {},
        );
        let app = plan.groups.iter().find(|g| g.app == "App").unwrap();
        assert_eq!(app.bytes, 100, "the excluded folder is not counted");
        assert!(app.on);
        let other = plan.groups.iter().find(|g| g.app == "Other").unwrap();
        assert_eq!(other.running.as_deref(), Some("Other.exe"));
        assert!(!other.on, "a target whose program runs is not pre-selected");

        let excluded = Rules {
            exclusions: vec![local.join("App")],
            ..Rules::default()
        };
        let plan = build_plan(&registry, &roots, &excluded, &|_| false, |_| {});
        let app = plan.groups.iter().find(|g| g.app == "App").unwrap();
        assert!(app.targets.is_empty() && app.bytes == 0);
    }

    #[test]
    fn unique_targets_is_case_insensitive_keeping_first() {
        let targets = ["/a/B/c", "/a/b/C", "/a/b/d"]
            .map(|path| Target::whole(PathBuf::from(path), 0))
            .into();
        let unique: Vec<_> = unique_targets(targets)
            .into_iter()
            .map(|target| target.path)
            .collect();
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
        let plan = build_plan(&registry, &roots, &Rules::default(), &|_| false, |_| {
            updates += 1;
        });
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
    fn scan_covers_multiple_profiles_file_globs_and_overlapping_targets() {
        let dir = tempfile::tempdir().unwrap();
        let roots = test_roots(dir.path());
        let local = roots.local_app_data.clone().unwrap();

        write_file(&local.join("Browser/Default/Cache/a.bin"), 10);
        write_file(&local.join("Browser/Profile 1/Cache/deep/b.bin"), 20);
        write_file(&local.join("Explorer/thumbcache_96.db"), 30);
        write_file(&local.join("Explorer/not-a-cache.db"), 99);

        let cache = local.join("Browser/Default/Cache");
        let registry = Registry {
            items: vec![
                Item::new("Browser", "profiles", true)
                    .paths([cache.clone(), cache.clone()])
                    .globs([local.join("Browser/*/Cache")]),
                Item::new("Windows", "thumbnail cache", true)
                    .globs([local.join("Explorer/thumbcache*.db")]),
            ],
        };

        let plan = build_plan(&registry, &roots, &Rules::default(), &|_| false, |_| {});
        let browser = plan
            .groups
            .iter()
            .find(|group| group.app == "Browser")
            .unwrap();
        assert_eq!(
            browser
                .targets
                .iter()
                .map(|t| t.path.clone())
                .collect::<Vec<_>>(),
            vec![cache, local.join("Browser/Profile 1/Cache")],
            "duplicate static and glob matches are de-duplicated case-insensitively"
        );
        assert_eq!(browser.bytes, 30);

        let thumbnails = plan
            .groups
            .iter()
            .find(|group| group.app == "Windows")
            .unwrap();
        assert_eq!(
            thumbnails
                .targets
                .iter()
                .map(|t| t.path.clone())
                .collect::<Vec<_>>(),
            vec![local.join("Explorer/thumbcache_96.db")]
        );
        assert_eq!(thumbnails.bytes, 30);
        assert_eq!(plan.total_bytes, 60);
        assert_eq!(plan.selected, 2);
    }

    #[test]
    fn scan_sums_deep_and_wide_trees_and_tolerates_missing_paths() {
        let dir = tempfile::tempdir().unwrap();
        let roots = test_roots(dir.path());
        let local = roots.local_app_data.clone().unwrap();
        let tree = local.join("Tree");

        for depth in 0..16 {
            write_file(
                &tree.join("deep").join("d".repeat(depth)).join("data.bin"),
                depth + 1,
            );
        }
        for index in 0..64 {
            write_file(&tree.join(format!("wide/{index:02}/data.bin")), 2);
        }
        let expected = (1..=16).sum::<usize>() + 64 * 2;
        let registry = Registry {
            items: vec![
                Item::new("Tree", "deep and wide", true).paths([tree]),
                Item::new("Missing", "absent", true).paths([local.join("Missing")]),
            ],
        };

        let plan = build_plan(&registry, &roots, &Rules::default(), &|_| false, |_| {});
        let tree = plan
            .groups
            .iter()
            .find(|group| group.app == "Tree")
            .unwrap();
        assert_eq!(tree.bytes, expected as u64);
        assert!(tree.errs.is_empty());
        let missing = plan
            .groups
            .iter()
            .find(|group| group.app == "Missing")
            .unwrap();
        assert_eq!(missing.bytes, 0);
        assert!(missing.errs.is_empty());
        assert!(!missing.on);
    }

    #[cfg(unix)]
    #[test]
    fn unreadable_tree_errors_are_tolerated_without_counting_bytes() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempfile::tempdir().unwrap();
        let locked = dir.path().join("locked");
        create_dir_all(&locked).unwrap();
        fs::set_permissions(&locked, fs::Permissions::from_mode(0o000)).unwrap();
        let size = dir_size(&locked);
        fs::set_permissions(&locked, fs::Permissions::from_mode(0o755)).unwrap();

        assert_eq!(size.unwrap(), 0);
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

        let plan = build_plan(&registry, &roots, &Rules::default(), &|_| false, |_| {});
        let evil = plan.groups.iter().find(|g| g.app == "Evil").unwrap();
        assert_eq!(evil.errs.len(), 1);
        assert!(evil.errs[0].contains("unsafe"));
        assert_eq!(evil.bytes, 0);
    }

    #[cfg(unix)]
    #[test]
    fn build_plan_rejects_paths_through_symlink_ancestors() {
        let dir = tempfile::tempdir().unwrap();
        let roots = test_roots(dir.path());
        let local = roots.local_app_data.clone().unwrap();
        let outside = dir.path().join("outside");
        write_file(&outside.join("cache/data.bin"), 128);
        create_dir_all(&local).unwrap();
        std::os::unix::fs::symlink(&outside, local.join("linked")).unwrap();

        let registry = Registry {
            items: vec![
                Item::new("Unsafe", "linked cache", true).paths([local.join("linked/cache")]),
            ],
        };
        let plan = build_plan(&registry, &roots, &Rules::default(), &|_| false, |_| {});
        let group = &plan.groups[0];

        assert_eq!(group.bytes, 0);
        assert!(!group.on);
        assert_eq!(group.errs.len(), 1);
        assert!(group.errs[0].contains("unsafe"));
    }
}
