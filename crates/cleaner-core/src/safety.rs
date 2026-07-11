//! The core safety invariant: a path may only be deleted when it is strictly
//! inside one of the known safe roots. The guard is applied when scanning and
//! re-checked immediately before every delete.

use std::path::{Component, MAIN_SEPARATOR, Path};

/// Reports whether `path` is strictly under one of `roots` (never a root
/// itself). Comparison is lexical, case-insensitive, on cleaned paths —
/// so the guard is conservative on every supported filesystem.
#[must_use]
pub fn is_safe_path(path: &Path, roots: &[&Path]) -> bool {
    roots.iter().any(|root| is_path_under_root(path, root))
}

fn is_path_under_root(path: &Path, root: &Path) -> bool {
    let path_key = normalized_key(path);
    let root_key = normalized_key(root);
    if path_key.is_empty() || root_key.is_empty() || path_key == root_key {
        return false;
    }
    path_key.starts_with(&format!("{root_key}{MAIN_SEPARATOR}"))
}

/// Builds the comparison key used by both the safety guard and path
/// de-duplication: a lexically cleaned path,
/// lowercased. Using one key function for both keeps them consistent.
#[must_use]
pub fn normalized_key(path: &Path) -> String {
    let mut parts: Vec<String> = Vec::new();
    let mut prefix = String::new();
    let mut rooted = false;

    for component in path.components() {
        match component {
            Component::Prefix(p) => prefix = p.as_os_str().to_string_lossy().into_owned(),
            Component::RootDir => rooted = true,
            Component::CurDir => {}
            Component::ParentDir => {
                if parts.pop().is_none() && !rooted {
                    parts.push("..".to_owned());
                }
            }
            Component::Normal(name) => parts.push(name.to_string_lossy().into_owned()),
        }
    }

    let mut key = prefix;
    if rooted {
        key.push(MAIN_SEPARATOR);
    }
    key.push_str(&parts.join(std::path::MAIN_SEPARATOR_STR));
    if key.is_empty() {
        key.push('.');
    }
    key.to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn sep(path: &str) -> PathBuf {
        PathBuf::from(path.replace('/', std::path::MAIN_SEPARATOR_STR))
    }

    #[test]
    fn root_itself_is_never_safe() {
        let root = sep("/home/user/appdata");
        assert!(!is_safe_path(&root, &[&root]));
    }

    #[test]
    fn strictly_inside_is_safe() {
        let root = sep("/home/user/appdata");
        assert!(is_safe_path(&sep("/home/user/appdata/x"), &[&root]));
        assert!(is_safe_path(&sep("/home/user/appdata/x/y/z"), &[&root]));
    }

    #[test]
    fn outside_and_sibling_prefixes_are_unsafe() {
        let root = sep("/home/user/appdata");
        assert!(!is_safe_path(&sep("/home/user/appdata2/x"), &[&root]));
        assert!(!is_safe_path(&sep("/home/user"), &[&root]));
        assert!(!is_safe_path(&sep("/elsewhere"), &[&root]));
    }

    #[test]
    fn comparison_is_case_insensitive() {
        let root = sep("/Home/User/AppData");
        assert!(is_safe_path(&sep("/home/user/appdata/X"), &[&root]));
    }

    #[test]
    fn dotdot_cannot_escape() {
        let root = sep("/home/user/appdata");
        assert!(!is_safe_path(&sep("/home/user/appdata/../other"), &[&root]));
        assert!(is_safe_path(&sep("/home/user/appdata/a/../b"), &[&root]));
    }

    #[test]
    fn empty_inputs_are_unsafe() {
        let root = sep("/home/user/appdata");
        assert!(!is_safe_path(Path::new(""), &[&root]));
        assert!(!is_safe_path(&sep("/home/user/appdata/x"), &[]));
    }

    #[test]
    fn normalized_key_cleans_lexically() {
        assert_eq!(
            normalized_key(&sep("/A/b/./c/../D")),
            normalized_key(&sep("/a/b/d"))
        );
        assert_eq!(normalized_key(Path::new("")), ".");
    }
}
