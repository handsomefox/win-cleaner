use std::path::{Path, PathBuf};

/// Filesystem roots the catalog and safety guard are built from. On Windows
/// these come from known folders and environment variables (resolved in
/// `cleaner-platform`); tests inject temp directories.
#[derive(Debug, Clone, Default)]
pub struct Roots {
    /// `%LOCALAPPDATA%`
    pub local_app_data: Option<PathBuf>,
    /// `%APPDATA%`
    pub roaming_app_data: Option<PathBuf>,
    /// `%PROGRAMDATA%`
    pub program_data: Option<PathBuf>,
    /// `%USERPROFILE%`
    pub user_profile: Option<PathBuf>,
    /// `%ProgramFiles(x86)%`
    pub program_files_x86: Option<PathBuf>,
    /// `%ProgramFiles%`
    pub program_files: Option<PathBuf>,
    /// `%ProgramW6432%`
    pub program_w6432: Option<PathBuf>,
    /// `%SystemRoot%`
    pub system_root: Option<PathBuf>,
    /// `%windir%`
    pub windir: Option<PathBuf>,
}

impl Roots {
    /// The four roots the catalog requires; when any is missing the catalog
    /// is empty on non-Windows platforms.
    #[must_use]
    pub fn has_required(&self) -> bool {
        self.local_app_data.is_some()
            && self.roaming_app_data.is_some()
            && self.program_data.is_some()
            && self.user_profile.is_some()
    }

    /// Every root a deletable path may live under. A path must be strictly
    /// inside one of these to pass the safety guard.
    #[must_use]
    pub fn guard_roots(&self) -> Vec<&Path> {
        [
            &self.local_app_data,
            &self.roaming_app_data,
            &self.program_data,
            &self.user_profile,
            &self.system_root,
            &self.windir,
            &self.program_files,
            &self.program_files_x86,
            &self.program_w6432,
        ]
        .into_iter()
        .filter_map(|root| root.as_deref())
        .collect()
    }

    /// `%USERPROFILE%\AppData\LocalLow`, when the profile root is known.
    #[must_use]
    pub fn local_low(&self) -> Option<PathBuf> {
        self.user_profile
            .as_ref()
            .map(|profile| profile.join("AppData").join("LocalLow"))
    }
}
