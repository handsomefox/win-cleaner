//! Minimal glob expansion for the patterns the catalog uses: `*` within a
//! single path component, everything else literal (including `#`, `(`, `)`,
//! `?`, `[`). Matching is case-insensitive to reflect Windows filesystem
//! semantics. `*` never crosses path separators, missing directories yield no
//! matches (never an error), and results are sorted.

use std::fs;
use std::path::{Component, Path, PathBuf};

use crate::safety::is_reparse_point;

/// Expands a pattern into the existing paths that match it.
#[must_use]
pub fn expand(pattern: &Path) -> Vec<PathBuf> {
    let mut candidates: Vec<PathBuf> = vec![PathBuf::new()];
    let mut had_wildcard = false;

    for component in pattern.components() {
        let text = component.as_os_str().to_string_lossy();
        if text.contains('*') {
            had_wildcard = true;
            let mut next = Vec::new();
            for base in &candidates {
                let Ok(entries) = fs::read_dir(base) else {
                    continue;
                };
                for entry in entries.flatten() {
                    let path = entry.path();
                    let Ok(metadata) = fs::symlink_metadata(&path) else {
                        continue;
                    };
                    if metadata.file_type().is_symlink() || is_reparse_point(&metadata) {
                        continue;
                    }
                    let name = entry.file_name();
                    if wildcard_match(&text, &name.to_string_lossy()) {
                        next.push(base.join(name));
                    }
                }
            }
            candidates = next;
        } else {
            // Root/prefix components (`/`, `C:\`) and normal names extend
            // every candidate unchanged.
            for base in &mut candidates {
                base.push(component);
            }
            // Prune candidates that no longer exist so `read_dir` calls and
            // the final existence filter stay cheap.
            if !matches!(component, Component::Prefix(_) | Component::RootDir) {
                candidates.retain(|candidate| fs::symlink_metadata(candidate).is_ok());
            }
        }
        if candidates.is_empty() {
            return Vec::new();
        }
    }

    if !had_wildcard {
        // A literal pattern matches only if it exists.
        candidates.retain(|candidate| fs::symlink_metadata(candidate).is_ok());
    }
    candidates.sort();
    candidates
}

/// Matches `name` against a single-component pattern where `*` matches any
/// run of characters (including none) and every other character is literal.
/// Matching is case-insensitive.
fn wildcard_match(pattern: &str, name: &str) -> bool {
    let pattern = pattern.to_lowercase();
    let name = name.to_lowercase();
    let pattern = pattern.as_str();
    let name = name.as_str();
    let mut parts = pattern.split('*');
    let Some(prefix) = parts.next() else {
        return pattern == name;
    };
    let Some(mut rest) = name.strip_prefix(prefix) else {
        return false;
    };

    let segments: Vec<&str> = parts.collect();
    if segments.is_empty() {
        // No `*` at all: the whole pattern was literal.
        return rest.is_empty();
    }

    let (middle, last) = segments.split_at(segments.len() - 1);
    for segment in middle {
        if segment.is_empty() {
            continue;
        }
        match rest.find(segment) {
            Some(index) => rest = &rest[index + segment.len()..],
            None => return false,
        }
    }
    rest.ends_with(last[0])
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::{File, create_dir_all};

    #[test]
    fn wildcard_match_semantics() {
        assert!(wildcard_match("user_data*", "user_data"));
        assert!(wildcard_match("user_data*", "user_data#2"));
        assert!(!wildcard_match("user_data*", "user-data"));
        assert!(wildcard_match("*default-release", "abc123.default-release"));
        assert!(!wildcard_match(
            "*default-release",
            "abc123.default-release2"
        ));
        assert!(wildcard_match("thumbcache*.db", "thumbcache_1280.db"));
        assert!(!wildcard_match("thumbcache*.db", "thumbcache_1280.db.tmp"));
        assert!(wildcard_match("*", "anything"));
        assert!(wildcard_match("literal", "literal"));
        assert!(wildcard_match("literal", "Literal"));
        assert!(wildcard_match("THUMBCACHE*.DB", "thumbcache_1280.db"));
        assert!(wildcard_match("a*b*c", "aXbYc"));
        assert!(!wildcard_match("a*b*c", "aXcYb"));
    }

    #[test]
    fn expands_telegram_style_accounts() {
        let dir = tempfile::tempdir().unwrap();
        let tdata = dir.path().join("tdata");
        create_dir_all(tdata.join("user_data").join("cache")).unwrap();
        create_dir_all(tdata.join("user_data#2").join("cache")).unwrap();
        create_dir_all(tdata.join("user_data#3")).unwrap(); // no cache subdir
        create_dir_all(tdata.join("unrelated").join("cache")).unwrap();

        let matches = expand(&tdata.join("user_data*").join("cache"));
        assert_eq!(
            matches,
            vec![
                tdata.join("user_data").join("cache"),
                tdata.join("user_data#2").join("cache"),
            ]
        );
    }

    #[test]
    fn star_does_not_cross_separators() {
        let dir = tempfile::tempdir().unwrap();
        create_dir_all(dir.path().join("a").join("b").join("Cache")).unwrap();
        create_dir_all(dir.path().join("c").join("Cache")).unwrap();

        let matches = expand(&dir.path().join("*").join("Cache"));
        assert_eq!(matches, vec![dir.path().join("c").join("Cache")]);
    }

    #[cfg(unix)]
    #[test]
    fn wildcard_does_not_descend_through_symlinked_directories() {
        let dir = tempfile::tempdir().unwrap();
        let outside = dir.path().join("outside");
        let profiles = dir.path().join("profiles");
        create_dir_all(outside.join("Cache")).unwrap();
        create_dir_all(&profiles).unwrap();
        std::os::unix::fs::symlink(&outside, profiles.join("linked-profile")).unwrap();

        let matches = expand(&profiles.join("*").join("Cache"));
        assert!(matches.is_empty());
    }

    #[test]
    fn missing_directory_yields_no_matches() {
        let dir = tempfile::tempdir().unwrap();
        let matches = expand(&dir.path().join("nope").join("*").join("Cache"));
        assert!(matches.is_empty());
    }

    #[test]
    fn literal_pattern_requires_existence() {
        let dir = tempfile::tempdir().unwrap();
        File::create(dir.path().join("present")).unwrap();
        assert_eq!(
            expand(&dir.path().join("present")),
            vec![dir.path().join("present")]
        );
        assert!(expand(&dir.path().join("absent")).is_empty());
    }

    #[test]
    fn matches_files_not_only_dirs() {
        let dir = tempfile::tempdir().unwrap();
        File::create(dir.path().join("thumbcache_96.db")).unwrap();
        File::create(dir.path().join("thumbcache_sr.db")).unwrap();
        File::create(dir.path().join("other.db")).unwrap();

        let matches = expand(&dir.path().join("thumbcache*.db"));
        assert_eq!(matches.len(), 2);
    }
}
