//! Exporting the settings to a file and importing them back, to keep a copy
//! or move them to another PC.
//!
//! The file carries the cleanup rules, the presets, and the selection
//! preferences. It never carries "always start as administrator": a file
//! someone hands you must not be able to turn that on. An import checks
//! everything before any of it applies, and starts no scan or cleanup.

use serde::{Deserialize, Serialize};

use cleaner_core::Rules;

use crate::presets::Presets;
use crate::viewmodel::TargetKey;

/// Names the file's format, so a stray JSON file is refused by name.
const FORMAT: &str = "win-cleaner-settings";
/// The newest version this build reads and the one it writes.
const VERSION: u32 = 1;
/// A settings file is a few kilobytes. Refuse anything far larger.
pub(crate) const MAX_BYTES: u64 = 4 * 1024 * 1024;

/// Everything a settings file holds.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct SettingsFile {
    pub format: String,
    pub version: u32,
    pub rules: Rules,
    pub presets: Presets,
    pub remember_selection: bool,
    pub show_empty: bool,
    pub selection: Vec<TargetKey>,
}

/// Why a settings file was not imported.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ImportError {
    TooLarge,
    NotJson(String),
    WrongFormat,
    NewerVersion(u32),
}

impl SettingsFile {
    pub(crate) fn new(
        rules: &Rules,
        presets: &Presets,
        remember_selection: bool,
        show_empty: bool,
        selection: &[TargetKey],
    ) -> Self {
        Self {
            format: FORMAT.to_owned(),
            version: VERSION,
            rules: rules.clone(),
            presets: presets.clone(),
            remember_selection,
            show_empty,
            selection: selection.to_vec(),
        }
    }

    pub(crate) fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_default()
    }

    /// Reads and checks a settings file. Exclusions that are not absolute
    /// paths are dropped, presets are cleaned the way storage is, and keys
    /// for targets from older versions are carried over.
    pub(crate) fn parse(bytes: &[u8]) -> Result<Self, ImportError> {
        if bytes.len() as u64 > MAX_BYTES {
            return Err(ImportError::TooLarge);
        }
        let mut file: Self = serde_json::from_slice(bytes)
            .map_err(|error| ImportError::NotJson(error.to_string()))?;
        if file.format != FORMAT {
            return Err(ImportError::WrongFormat);
        }
        if file.version > VERSION {
            return Err(ImportError::NewerVersion(file.version));
        }
        file.rules.sanitize();
        file.presets.sanitize();
        file.presets.migrate_keys();
        crate::migrate::target_keys(&mut file.selection);
        Ok(file)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn sample() -> SettingsFile {
        let mut presets = Presets::default();
        presets
            .save(
                "Weekly",
                vec![("Chrome".to_owned(), "Profile cache".to_owned())],
            )
            .unwrap();
        SettingsFile::new(
            &Rules {
                exclusions: vec![std::env::temp_dir().join("keep-this")],
                min_age_days: 7,
                min_size_bytes: 1024,
                skip_running_apps: true,
            },
            &presets,
            true,
            false,
            &[("Steam".to_owned(), "HTML cache".to_owned())],
        )
    }

    #[test]
    fn a_file_round_trips() {
        let file = sample();
        assert_eq!(SettingsFile::parse(file.to_json().as_bytes()), Ok(file));
    }

    #[test]
    fn foreign_newer_and_oversized_files_are_refused() {
        assert!(matches!(
            SettingsFile::parse(br#"{"format":"something-else","version":1}"#),
            Err(ImportError::NotJson(_))
        ));
        let mut other = sample();
        other.format = "another-app".to_owned();
        assert_eq!(
            SettingsFile::parse(other.to_json().as_bytes()),
            Err(ImportError::WrongFormat)
        );
        let mut newer = sample();
        newer.version = 2;
        assert_eq!(
            SettingsFile::parse(newer.to_json().as_bytes()),
            Err(ImportError::NewerVersion(2))
        );
        assert!(matches!(
            SettingsFile::parse(b"not json"),
            Err(ImportError::NotJson(_))
        ));
        let huge = vec![b' '; usize::try_from(MAX_BYTES).unwrap() + 1];
        assert_eq!(SettingsFile::parse(&huge), Err(ImportError::TooLarge));
    }

    #[test]
    fn imported_paths_and_keys_are_checked() {
        let mut file = sample();
        file.rules.exclusions.push(PathBuf::from("relative/path"));
        file.selection
            .push(("Chrome Beta".to_owned(), "Profile cache".to_owned()));
        let imported = SettingsFile::parse(file.to_json().as_bytes()).unwrap();
        assert_eq!(
            imported.rules.exclusions,
            vec![std::env::temp_dir().join("keep-this")]
        );
        assert!(
            imported
                .selection
                .contains(&("Chrome".to_owned(), "Profile cache (Beta)".to_owned()))
        );
    }

    #[test]
    fn the_file_never_carries_the_administrator_setting() {
        let json = sample().to_json();
        assert!(!json.contains("admin"));
        assert!(!json.contains("elevat"));
    }
}
