//! Old program versions that Blizzard's updater leaves behind.
//!
//! The Battle.net launcher installs each build into `Battle.net.<build>` and
//! its Agent into `Agent.<build>`, beside a `.product.db` that records the
//! installed version as `<major>.<minor>.<patch>.<build>`, such as
//! `2.41.2.9852` for `Agent.9852`. Superseded builds can stay behind.
//!
//! These are program files, not regenerable caches, so the rules are strict:
//!
//! - The build `.product.db` names is never returned, and neither is the
//!   newest build.
//! - When `.product.db` is missing, unreadable, or names no build that exists
//!   as a folder, nothing is returned: without it, nothing says which folder
//!   is running.
//! - Only real folders named exactly `<prefix><digits>` are considered.
//!
//! Cleanup asks again immediately before it moves anything, because an update
//! between the scan and the cleanup can change which build is installed.

use std::fs;
use std::io::Read as _;
use std::path::PathBuf;

use crate::catalog::Versioned;
use crate::safety::is_link;

/// The file beside the version folders that names the installed version.
pub const PRODUCT_DB: &str = ".product.db";

/// `.product.db` is a few hundred bytes. Anything far larger is not one.
const MAX_PRODUCT_DB_BYTES: u64 = 1024 * 1024;

/// The build numbers of every `<major>.<minor>.<patch>.<build>` version
/// written in `bytes`, in order of appearance.
///
/// `.product.db` is a protocol buffer, but the version is stored as plain
/// ASCII, so a scan for the four-number pattern finds it without decoding
/// the format.
#[must_use]
pub fn installed_builds(bytes: &[u8]) -> Vec<u64> {
    let mut builds = Vec::new();
    for run in bytes.split(|byte| !(byte.is_ascii_digit() || *byte == b'.')) {
        let Ok(text) = std::str::from_utf8(run) else {
            continue;
        };
        let parts: Vec<&str> = text.split('.').collect();
        if parts.len() == 4
            && parts.iter().all(|part| !part.is_empty() && part.len() <= 9)
            && let Ok(build) = parts[3].parse::<u64>()
        {
            builds.push(build);
        }
    }
    builds
}

/// The build number in a folder name of the form `<prefix><digits>`.
fn folder_build(name: &str, prefix: &str) -> Option<u64> {
    let digits = name.strip_prefix(prefix)?;
    if digits.is_empty() || digits.len() > 9 || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    digits.parse().ok()
}

/// The superseded version folders the rule finds, oldest first. Empty unless
/// `.product.db` names a build that exists as a folder.
#[must_use]
pub fn superseded(rule: &Versioned) -> Vec<PathBuf> {
    let Some(installed) = read_product_db(rule) else {
        return Vec::new();
    };
    let Ok(entries) = fs::read_dir(&rule.dir) else {
        return Vec::new();
    };
    let mut folders: Vec<(u64, PathBuf)> = entries
        .flatten()
        .filter_map(|entry| {
            let metadata = fs::symlink_metadata(entry.path()).ok()?;
            if !metadata.is_dir() || is_link(&metadata) {
                return None;
            }
            let name = entry.file_name();
            let build = folder_build(name.to_str()?, &rule.prefix)?;
            Some((build, entry.path()))
        })
        .collect();

    if !folders.iter().any(|(build, _)| installed.contains(build)) {
        return Vec::new();
    }
    let newest = folders.iter().map(|(build, _)| *build).max();
    folders.retain(|(build, _)| !installed.contains(build) && Some(*build) != newest);
    folders.sort_by_key(|(build, _)| *build);
    folders.into_iter().map(|(_, path)| path).collect()
}

fn read_product_db(rule: &Versioned) -> Option<Vec<u64>> {
    let path = rule.dir.join(PRODUCT_DB);
    let metadata = fs::symlink_metadata(&path).ok()?;
    if !metadata.is_file() || metadata.len() > MAX_PRODUCT_DB_BYTES {
        return None;
    }
    let mut bytes = Vec::new();
    fs::File::open(&path)
        .ok()?
        .take(MAX_PRODUCT_DB_BYTES)
        .read_to_end(&mut bytes)
        .ok()?;
    let builds = installed_builds(&bytes);
    (!builds.is_empty()).then_some(builds)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::{create_dir_all, write};
    use std::path::Path;

    /// The bytes a real Agent `.product.db` holds around its version.
    fn product_db(version: &str) -> Vec<u8> {
        let mut bytes =
            b"\n\x05agent\x12\x05agent\x1a!C:/ProgramData/Battle.net/Agent\"\x04UKRb\x0c".to_vec();
        bytes.extend_from_slice(version.as_bytes());
        bytes.extend_from_slice(b"R\x00 26000534d51b737e111bd408814c836d2");
        bytes
    }

    fn agent(root: &Path, folders: &[&str], version: Option<&str>) -> Versioned {
        for name in folders {
            create_dir_all(root.join(name)).unwrap();
        }
        if let Some(version) = version {
            write(root.join(PRODUCT_DB), product_db(version)).unwrap();
        }
        Versioned {
            dir: root.to_path_buf(),
            prefix: "Agent.".to_owned(),
        }
    }

    #[test]
    fn reads_the_installed_build_out_of_product_db() {
        assert_eq!(installed_builds(&product_db("2.41.2.9852")), vec![9852]);
        assert_eq!(installed_builds(b"2.53.8.17986R"), vec![17986]);
        // A hash, a date, and a three-part version are not a build.
        assert!(installed_builds(b"26000534d51b737e 2026.11.11 1.2.3 ..1.2").is_empty());
    }

    #[test]
    fn returns_every_build_but_the_installed_and_the_newest() {
        let temp = tempfile::tempdir().unwrap();
        let rule = agent(
            temp.path(),
            &["Agent.9700", "Agent.9775", "Agent.9852", "Agent.9900"],
            Some("2.41.2.9852"),
        );
        assert_eq!(
            superseded(&rule),
            vec![
                temp.path().join("Agent.9700"),
                temp.path().join("Agent.9775")
            ],
            "the installed build and a newer one, perhaps pending, both stay"
        );
    }

    #[test]
    fn returns_nothing_without_a_product_db_that_names_a_folder() {
        let temp = tempfile::tempdir().unwrap();
        let rule = agent(temp.path(), &["Agent.9700", "Agent.9852"], None);
        assert!(superseded(&rule).is_empty(), "no .product.db");

        write(temp.path().join(PRODUCT_DB), b"not a version").unwrap();
        assert!(superseded(&rule).is_empty(), "no version in it");

        write(temp.path().join(PRODUCT_DB), product_db("2.41.2.9999")).unwrap();
        assert!(
            superseded(&rule).is_empty(),
            "the named build has no folder, so which one runs is unknown"
        );
    }

    #[test]
    fn ignores_names_that_are_not_exactly_prefix_and_digits() {
        let temp = tempfile::tempdir().unwrap();
        let rule = agent(
            temp.path(),
            &[
                "Agent.9700",
                "Agent.9852",
                "Agent.9700.bak",
                "Agent.",
                "Agent9600",
                "Logs",
            ],
            Some("2.41.2.9852"),
        );
        write(temp.path().join("Agent.9500"), b"a file, not a folder").unwrap();
        assert_eq!(superseded(&rule), vec![temp.path().join("Agent.9700")]);
    }

    #[cfg(unix)]
    #[test]
    fn never_returns_a_linked_version_folder() {
        let temp = tempfile::tempdir().unwrap();
        let elsewhere = tempfile::tempdir().unwrap();
        let rule = agent(temp.path(), &["Agent.9852"], Some("2.41.2.9852"));
        std::os::unix::fs::symlink(elsewhere.path(), temp.path().join("Agent.9700")).unwrap();
        assert!(superseded(&rule).is_empty());
    }
}
