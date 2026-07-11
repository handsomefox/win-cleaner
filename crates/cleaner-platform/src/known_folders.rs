//! `SHGetKnownFolderPath` wrappers for the four user roots. Preferred over
//! raw environment variables per Windows conventions; callers fall back to
//! env vars when a lookup fails.

use std::path::PathBuf;

use windows::Win32::System::Com::CoTaskMemFree;
use windows::Win32::UI::Shell::{
    FOLDERID_LocalAppData, FOLDERID_Profile, FOLDERID_ProgramData, FOLDERID_RoamingAppData,
    KNOWN_FOLDER_FLAG, SHGetKnownFolderPath,
};

pub(crate) fn local_app_data() -> Option<PathBuf> {
    known_folder(&FOLDERID_LocalAppData)
}

pub(crate) fn roaming_app_data() -> Option<PathBuf> {
    known_folder(&FOLDERID_RoamingAppData)
}

pub(crate) fn program_data() -> Option<PathBuf> {
    known_folder(&FOLDERID_ProgramData)
}

pub(crate) fn profile() -> Option<PathBuf> {
    known_folder(&FOLDERID_Profile)
}

fn known_folder(id: &windows::core::GUID) -> Option<PathBuf> {
    // SAFETY: the returned COM allocation is converted to an owned string
    // before being freed exactly once.
    unsafe {
        let value = SHGetKnownFolderPath(id, KNOWN_FOLDER_FLAG::default(), None).ok()?;
        let path = value.to_string().ok().map(PathBuf::from);
        CoTaskMemFree(Some(value.as_ptr().cast()));
        path
    }
}
