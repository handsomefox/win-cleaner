//! Resolution of the filesystem roots and application data directories.
//!
//! The four user roots come from `app-folders`, which asks
//! `SHGetKnownFolderPath` and falls back to environment variables, while the
//! Windows/Program Files guard roots keep their environment-variable semantics
//! on purpose — they mirror what the catalog and safety guard were designed
//! around.

use std::path::PathBuf;

use cleaner_core::Roots;

/// The app's folder under `%LOCALAPPDATA%`. Users' run history lives in it, so
/// it must never change.
const APP_DIR: &str = "win-cleaner";

/// Resolves the roots the catalog and safety guard operate on. On
/// non-Windows platforms everything is `None`, which yields an empty catalog.
#[must_use]
pub fn resolve_roots() -> Roots {
    #[cfg(windows)]
    {
        fn env_path(name: &str) -> Option<PathBuf> {
            std::env::var_os(name)
                .filter(|value| !value.is_empty())
                .map(PathBuf::from)
        }

        Roots {
            local_app_data: app_folders::local_app_data(),
            roaming_app_data: app_folders::roaming_app_data(),
            program_data: app_folders::program_data(),
            user_profile: app_folders::profile(),
            program_files_x86: env_path("ProgramFiles(x86)"),
            program_files: env_path("ProgramFiles"),
            program_w6432: env_path("ProgramW6432"),
            system_root: env_path("SystemRoot"),
            windir: env_path("windir"),
        }
    }
    #[cfg(not(windows))]
    {
        Roots::default()
    }
}

/// Directory run statistics are written to and read from:
/// `%LOCALAPPDATA%\win-cleaner\stats` on Windows.
#[must_use]
pub fn stats_dir() -> Option<PathBuf> {
    app_folders::app_data_dir(APP_DIR).map(|dir| dir.join("stats"))
}

/// Directory the diagnostics log is written to:
/// `%LOCALAPPDATA%\win-cleaner\logs` on Windows.
#[must_use]
pub fn logs_dir() -> Option<PathBuf> {
    app_folders::app_data_dir(APP_DIR).map(|dir| dir.join("logs"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn data_dirs_nest_under_the_app_data_dir() {
        if let Some(app_dir) = app_folders::app_data_dir("win-cleaner") {
            assert_eq!(stats_dir(), Some(app_dir.join("stats")));
            assert_eq!(logs_dir(), Some(app_dir.join("logs")));
        }
    }
}
