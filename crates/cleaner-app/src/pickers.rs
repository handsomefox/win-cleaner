//! Native file pickers for exclusions and settings files.
//!
//! Each blocks the window while the dialog is open, as Windows' own dialogs
//! do. Off Windows there is no picker and each returns `None`; the app ships
//! for Windows only, and Linux builds exist for its tests.

use std::path::PathBuf;

/// A folder to exclude from cleanup.
pub(crate) fn pick_folder(title: &str) -> Option<PathBuf> {
    #[cfg(windows)]
    {
        rfd::FileDialog::new().set_title(title).pick_folder()
    }
    #[cfg(not(windows))]
    {
        let _ = title;
        None
    }
}

/// A file to exclude from cleanup.
pub(crate) fn pick_file(title: &str) -> Option<PathBuf> {
    #[cfg(windows)]
    {
        rfd::FileDialog::new().set_title(title).pick_file()
    }
    #[cfg(not(windows))]
    {
        let _ = title;
        None
    }
}

/// Where to write an exported settings file.
pub(crate) fn save_settings_file(title: &str) -> Option<PathBuf> {
    #[cfg(windows)]
    {
        rfd::FileDialog::new()
            .set_title(title)
            .set_file_name("win-cleaner-settings.json")
            .add_filter("Settings", &["json"])
            .save_file()
    }
    #[cfg(not(windows))]
    {
        let _ = title;
        None
    }
}

/// A settings file to import.
pub(crate) fn open_settings_file(title: &str) -> Option<PathBuf> {
    #[cfg(windows)]
    {
        rfd::FileDialog::new()
            .set_title(title)
            .add_filter("Settings", &["json"])
            .pick_file()
    }
    #[cfg(not(windows))]
    {
        let _ = title;
        None
    }
}
