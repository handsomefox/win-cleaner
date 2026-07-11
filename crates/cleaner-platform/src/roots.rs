//! Resolution of the filesystem roots and application data directories.
//!
//! The four user roots come from `SHGetKnownFolderPath` (falling back to
//! environment variables), while the Windows/Program Files guard roots keep
//! their environment-variable semantics on purpose — they mirror what the
//! catalog and safety guard were designed around.

use std::path::PathBuf;

use cleaner_core::Roots;

fn env_path(name: &str) -> Option<PathBuf> {
    std::env::var_os(name)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}

/// Resolves the roots the catalog and safety guard operate on. On
/// non-Windows platforms everything is `None`, which yields an empty catalog.
#[must_use]
pub fn resolve_roots() -> Roots {
    #[cfg(windows)]
    {
        use crate::known_folders;
        Roots {
            local_app_data: known_folders::local_app_data().or_else(|| env_path("LOCALAPPDATA")),
            roaming_app_data: known_folders::roaming_app_data().or_else(|| env_path("APPDATA")),
            program_data: known_folders::program_data().or_else(|| env_path("PROGRAMDATA")),
            user_profile: known_folders::profile().or_else(|| env_path("USERPROFILE")),
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

/// The application's data directory: `%LOCALAPPDATA%\win-cleaner` on Windows,
/// `~/.win-cleaner` elsewhere (useful when developing the GUI on Linux).
#[must_use]
pub fn app_data_dir() -> Option<PathBuf> {
    if let Some(local) = resolve_roots().local_app_data {
        return Some(local.join("win-cleaner"));
    }
    home_dir().map(|home| home.join(".win-cleaner"))
}

/// Directory run statistics are written to and read from.
#[must_use]
pub fn stats_dir() -> Option<PathBuf> {
    app_data_dir().map(|dir| dir.join("stats"))
}

/// Directory the diagnostics log is written to.
#[must_use]
pub fn logs_dir() -> Option<PathBuf> {
    app_data_dir().map(|dir| dir.join("logs"))
}

fn home_dir() -> Option<PathBuf> {
    env_path("USERPROFILE").or_else(|| env_path("HOME"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn data_dirs_nest_under_the_app_data_dir() {
        if let Some(app_dir) = app_data_dir() {
            assert_eq!(stats_dir(), Some(app_dir.join("stats")));
            assert_eq!(logs_dir(), Some(app_dir.join("logs")));
        }
    }
}
