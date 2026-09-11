//! Resolution for self-updating apps that leave their previous version on
//! disk: Discord keeps `app-1.0.9256` beside the live `app-1.0.9257`,
//! Battle.net keeps `Agent.9775`, and Velopack keeps the previous `.nupkg`.
//!
//! These are program files, not regenerable caches, so the rules are strict.
//! Ordering comes from the numbers in the name, never from a timestamp: a
//! repair or a reinstall can touch the older directory last. A name without a
//! version is never returned, and the newest match is always kept.

use std::path::{Path, PathBuf};

use crate::globs::expand;

/// The version numbers in `name`, in order: `app-1.0.9256` gives
/// `[1, 0, 9256]` and `Agent.9775` gives `[9775]`.
///
/// A number counts only where it stands on its own, between separators or at
/// either end of the name. A digit run touching a letter means the name is a
/// hash or a word rather than a version (`5bde10fb…`, `v1.2`), and the whole
/// name returns `None`. A name with no digits returns `None` too. Callers skip
/// those names instead of guessing where they sort.
#[must_use]
pub fn version_key(name: &str) -> Option<Vec<u64>> {
    let bytes = name.as_bytes();
    let mut key = Vec::new();
    let mut index = 0;
    while index < bytes.len() {
        if !bytes[index].is_ascii_digit() {
            index += 1;
            continue;
        }
        let start = index;
        while index < bytes.len() && bytes[index].is_ascii_digit() {
            index += 1;
        }
        let touches_letter = (start > 0 && bytes[start - 1].is_ascii_alphabetic())
            || bytes.get(index).is_some_and(u8::is_ascii_alphabetic);
        if touches_letter {
            return None;
        }
        // A run too long to be a version number is not one.
        key.push(name[start..index].parse().ok()?);
    }
    (!key.is_empty()).then_some(key)
}

/// The superseded matches of `pattern`: every match but the newest `keep`.
///
/// Matches whose name carries no version are never returned, and `keep` is
/// treated as at least one, so the newest version survives a catalog typo.
/// Fewer matches than that yields nothing.
#[must_use]
pub fn superseded(pattern: &Path, keep: usize) -> Vec<PathBuf> {
    let keep = keep.max(1);
    let mut matches: Vec<(Vec<u64>, PathBuf)> = expand(pattern)
        .into_iter()
        .filter_map(|path| {
            let name = path.file_name()?.to_str()?;
            Some((version_key(name)?, path))
        })
        .collect();
    if matches.len() <= keep {
        return Vec::new();
    }
    matches.sort_by(|(left, _), (right, _)| left.cmp(right));
    matches.truncate(matches.len() - keep);
    matches.into_iter().map(|(_, path)| path).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::{File, create_dir_all};

    #[test]
    fn version_key_reads_every_number_in_order() {
        assert_eq!(version_key("app-1.0.9256"), Some(vec![1, 0, 9256]));
        assert_eq!(version_key("Agent.9775"), Some(vec![9775]));
        assert_eq!(
            version_key("osulazer-2026.804.2-lazer-full.nupkg"),
            Some(vec![2026, 804, 2])
        );
        // Nothing to sort by.
        assert_eq!(version_key("RELEASES"), None);
        assert_eq!(version_key("SquirrelTemp"), None);
        // A hash is not a version, even though it splits into runs of digits
        // that each parse. Discord's download folder is full of these.
        assert_eq!(
            version_key("5bde10fbfeba85bbde30292052a8531437bb229985b3c00a4e6d19a71ee6136d"),
            None
        );
        // Digits glued to letters are a word or a build id, not a version.
        assert_eq!(version_key("v1.2.3"), None);
        assert_eq!(version_key("Update2.exe"), None);
        assert_eq!(version_key("{70316E2B-A36C}v0.100.2"), None);
    }

    #[test]
    fn version_key_compares_numerically() {
        // The string order would put "app-1.0.10" first, which would mark the
        // newest build as superseded.
        assert!(version_key("app-1.0.10") > version_key("app-1.0.9"));
    }

    fn dir(root: &Path, names: &[&str]) {
        for name in names {
            create_dir_all(root.join(name)).unwrap();
        }
    }

    #[test]
    fn superseded_returns_every_version_but_the_newest() {
        let temp = tempfile::tempdir().unwrap();
        dir(
            temp.path(),
            &["app-1.0.9254", "app-1.0.9256", "app-1.0.9257"],
        );

        let old = superseded(&temp.path().join("app-*"), 1);
        assert_eq!(
            old,
            vec![
                temp.path().join("app-1.0.9254"),
                temp.path().join("app-1.0.9256")
            ],
            "the live version must never be offered"
        );
    }

    #[test]
    fn superseded_keeps_quiet_without_an_older_version() {
        let temp = tempfile::tempdir().unwrap();
        dir(temp.path(), &["app-1.0.9257"]);
        assert!(superseded(&temp.path().join("app-*"), 1).is_empty());

        // Asking to keep more than exist is a no-op too.
        dir(temp.path(), &["app-1.0.9256"]);
        assert!(superseded(&temp.path().join("app-*"), 5).is_empty());

        // Nothing matches at all.
        assert!(superseded(&temp.path().join("absent-*"), 1).is_empty());
    }

    #[test]
    fn superseded_skips_names_without_a_version() {
        let temp = tempfile::tempdir().unwrap();
        let packages = temp.path().join("packages");
        create_dir_all(&packages).unwrap();
        for name in [
            "Discord-1.0.9254-full.nupkg",
            "Discord-1.0.9257-full.nupkg",
            "RELEASES",
        ] {
            File::create(packages.join(name)).unwrap();
        }
        create_dir_all(packages.join("SquirrelTemp")).unwrap();

        let old = superseded(&packages.join("*"), 1);
        assert_eq!(
            old,
            vec![packages.join("Discord-1.0.9254-full.nupkg")],
            "unversioned entries are never deleted"
        );
    }

    #[test]
    fn superseded_never_empties_a_folder_on_a_zero_keep() {
        let temp = tempfile::tempdir().unwrap();
        dir(temp.path(), &["Agent.9775"]);
        assert!(superseded(&temp.path().join("Agent.*"), 0).is_empty());
    }
}
