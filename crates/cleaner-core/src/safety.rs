//! The core safety invariant: a path may only be deleted when it is strictly
//! inside one of the known safe roots. The guard is applied when scanning and
//! re-checked immediately before every delete.
//!
//! The guard itself is `app-paths`, shared with the other apps. These tests
//! pin the behaviour this app relies on.

pub use app_paths::{is_link, is_reparse_point, is_safe_path, normalized_key};

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::create_dir_all;
    use std::path::{Path, PathBuf};

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

    #[cfg(windows)]
    #[test]
    fn comparison_is_case_insensitive() {
        let root = sep("/Home/User/AppData");
        assert!(is_safe_path(&sep("/home/user/appdata/X"), &[&root]));
    }

    #[test]
    fn dotdot_cannot_escape() {
        let root = sep("/home/user/appdata");
        assert!(!is_safe_path(&sep("/home/user/appdata/../other"), &[&root]));
        // Any `..` is refused, even one that stays inside the root.
        assert!(!is_safe_path(&sep("/home/user/appdata/a/../b"), &[&root]));
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
            normalized_key(&sep("/a/b/./c/../d")),
            normalized_key(&sep("/a/b/d"))
        );
        assert_eq!(normalized_key(Path::new("")), ".");
    }

    #[test]
    fn glob_pattern_ancestors_with_wildcards_are_safe() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("root");
        create_dir_all(root.join("sub")).unwrap(); // parent EXISTS → Windows ERROR_INVALID_NAME path
        assert!(is_safe_path(
            &root.join("sub").join("*").join("Cache"),
            &[&root]
        ));
        // and with a missing parent (Windows ERROR_PATH_NOT_FOUND path):
        assert!(is_safe_path(
            &root.join("missing").join("*").join("Cache"),
            &[&root]
        ));
    }

    #[cfg(unix)]
    #[test]
    fn symlink_ancestor_cannot_escape_the_root() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("safe");
        let outside = dir.path().join("outside");
        create_dir_all(outside.join("cache")).unwrap();
        create_dir_all(&root).unwrap();
        std::os::unix::fs::symlink(&outside, root.join("linked")).unwrap();

        assert!(!is_safe_path(&root.join("linked/cache"), &[&root]));
        assert!(!is_safe_path(&root.join("linked"), &[&root]));
    }
}
