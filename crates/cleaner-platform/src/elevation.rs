//! Running with administrator rights.
//!
//! The app starts unelevated (`asInvoker`). Targets under Program Files and
//! Windows need administrator rights, so the user can start a second copy
//! elevated. UAC can elevate as a different account when the signed-in one
//! is not an administrator, and that copy would resolve another account's
//! user folders, so [`elevated_as_someone_else`] lets the app refuse to
//! clean there.

use std::path::Path;

/// The argument an elevated copy is started with, so it never asks for
/// elevation again even when the setting to always start elevated is on.
pub const ELEVATED_ARG: &str = "--elevated";

/// Whether this process runs with administrator rights.
#[must_use]
pub fn is_elevated() -> bool {
    #[cfg(windows)]
    {
        imp::is_elevated()
    }
    #[cfg(not(windows))]
    {
        false
    }
}

/// When this elevated process runs as a different account than the one
/// signed in to the session, both names as `DOMAIN\user`: this process's
/// first, the session's second.
///
/// The accounts are compared by SID, since two APIs can spell one account's
/// name differently. `None` when the process is not elevated, the accounts
/// match, or anything cannot be read: an uncertain answer never blocks the
/// app.
#[must_use]
pub fn elevated_as_someone_else() -> Option<(String, String)> {
    #[cfg(windows)]
    {
        if !is_elevated() {
            return None;
        }
        let process = imp::process_account();
        let session = imp::session_account();
        tracing::info!(?process, ?session, "elevated; checking the account");
        let (process, session) = (process?, session?);
        match imp::same_account(&session) {
            Some(false) => Some((process, session)),
            Some(true) => None,
            None => {
                tracing::warn!("could not compare the accounts; allowing the scan");
                None
            }
        }
    }
    #[cfg(not(windows))]
    {
        None
    }
}

/// The file whose presence means "always start with administrator rights".
/// It sits beside the logs, so `main` can read it before the window opens,
/// and outside eframe's storage, which a settings import could not reach
/// anyway.
fn always_elevated_flag() -> Option<std::path::PathBuf> {
    app_folders::app_data_dir(crate::roots::APP_DIR).map(|dir| dir.join("always-elevated"))
}

/// Whether the user asked to always start with administrator rights.
#[must_use]
pub fn always_elevated() -> bool {
    always_elevated_flag().is_some_and(|flag| flag.is_file())
}

/// Turns "always start with administrator rights" on or off.
///
/// # Errors
///
/// Returns the error when the app's data folder cannot be found or the flag
/// cannot be written or removed.
pub fn set_always_elevated(on: bool) -> std::io::Result<()> {
    let flag = always_elevated_flag()
        .ok_or_else(|| std::io::Error::other("the app's data folder could not be found"))?;
    if on {
        if let Some(dir) = flag.parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::write(&flag, b"")
    } else {
        match std::fs::remove_file(&flag) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            other => other,
        }
    }
}

/// What asking Windows to start an elevated copy came to.
#[derive(Debug, PartialEq, Eq)]
pub enum Elevation {
    Started,
    /// The user answered No to the UAC prompt.
    Declined,
    Failed(String),
}

/// Starts `executable` with administrator rights through the UAC prompt,
/// passing [`ELEVATED_ARG`].
#[must_use]
pub fn start_elevated(executable: &Path) -> Elevation {
    #[cfg(windows)]
    {
        imp::start_elevated(executable)
    }
    #[cfg(not(windows))]
    {
        let _ = executable;
        Elevation::Failed("administrator rights are only available on Windows".to_owned())
    }
}

#[cfg(windows)]
mod imp {
    use std::ffi::c_void;
    use std::os::windows::ffi::OsStrExt as _;
    use std::path::Path;

    use windows::Win32::Foundation::{CloseHandle, ERROR_CANCELLED, HANDLE};
    use windows::Win32::Security::{
        EqualSid, GetTokenInformation, LookupAccountNameW, LookupAccountSidW, PSID, SID_NAME_USE,
        TOKEN_ELEVATION, TOKEN_QUERY, TOKEN_USER, TokenElevation, TokenUser,
    };
    use windows::Win32::System::Com::{
        COINIT_APARTMENTTHREADED, COINIT_DISABLE_OLE1DDE, CoInitializeEx, CoUninitialize,
    };
    use windows::Win32::System::RemoteDesktop::{
        WTS_CURRENT_SERVER_HANDLE, WTS_CURRENT_SESSION, WTS_INFO_CLASS, WTSDomainName,
        WTSFreeMemory, WTSQuerySessionInformationW, WTSUserName,
    };
    use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};
    use windows::Win32::UI::Shell::{SHELLEXECUTEINFOW, ShellExecuteExW};
    use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
    use windows::core::{PCWSTR, PWSTR, w};

    use super::{ELEVATED_ARG, Elevation};

    /// This process's token, closed on drop.
    struct Token(HANDLE);

    impl Token {
        fn open() -> Option<Self> {
            let mut handle = HANDLE::default();
            // SAFETY: returns a pseudo-handle that needs no closing.
            let process = unsafe { GetCurrentProcess() };
            // SAFETY: `process` is valid and `handle` is a valid out-pointer.
            unsafe { OpenProcessToken(process, TOKEN_QUERY, &raw mut handle) }.ok()?;
            Some(Self(handle))
        }

        /// Reads one token information class into a buffer of `size` bytes.
        fn read(
            &self,
            class: windows::Win32::Security::TOKEN_INFORMATION_CLASS,
        ) -> Option<Vec<u8>> {
            let mut needed = 0u32;
            // SAFETY: a null buffer with length zero asks for the size only.
            let _ = unsafe { GetTokenInformation(self.0, class, None, 0, &raw mut needed) };
            if needed == 0 {
                return None;
            }
            let mut buffer = vec![0u8; needed as usize];
            // SAFETY: `buffer` holds `needed` writable bytes.
            unsafe {
                GetTokenInformation(
                    self.0,
                    class,
                    Some(buffer.as_mut_ptr().cast::<c_void>()),
                    needed,
                    &raw mut needed,
                )
            }
            .ok()?;
            Some(buffer)
        }
    }

    impl Drop for Token {
        fn drop(&mut self) {
            // SAFETY: the handle came from OpenProcessToken and is closed once.
            let _ = unsafe { CloseHandle(self.0) };
        }
    }

    pub(super) fn is_elevated() -> bool {
        let Some(token) = Token::open() else {
            return false;
        };
        let Some(buffer) = token.read(TokenElevation) else {
            return false;
        };
        if buffer.len() < size_of::<TOKEN_ELEVATION>() {
            return false;
        }
        // SAFETY: the buffer holds at least one TOKEN_ELEVATION, written by
        // GetTokenInformation; read_unaligned copes with the Vec's alignment.
        let elevation: TOKEN_ELEVATION =
            unsafe { std::ptr::read_unaligned(buffer.as_ptr().cast()) };
        elevation.TokenIsElevated != 0
    }

    /// The account this process runs as, as `DOMAIN\user`.
    pub(super) fn process_account() -> Option<String> {
        let token = Token::open()?;
        let buffer = token.read(TokenUser)?;
        if buffer.len() < size_of::<TOKEN_USER>() {
            return None;
        }
        // SAFETY: as in `is_elevated`; the SID it points to lives in `buffer`,
        // which outlives every use below.
        let user: TOKEN_USER = unsafe { std::ptr::read_unaligned(buffer.as_ptr().cast()) };
        let mut name = [0u16; 256];
        let mut domain = [0u16; 256];
        let mut name_len = u32::try_from(name.len()).ok()?;
        let mut domain_len = u32::try_from(domain.len()).ok()?;
        let mut kind = SID_NAME_USE::default();
        // SAFETY: both buffers and their lengths are valid for writing.
        unsafe {
            LookupAccountSidW(
                PCWSTR::null(),
                user.User.Sid,
                Some(PWSTR(name.as_mut_ptr())),
                &raw mut name_len,
                Some(PWSTR(domain.as_mut_ptr())),
                &raw mut domain_len,
                &raw mut kind,
            )
        }
        .ok()?;
        Some(format!(
            "{}\\{}",
            String::from_utf16_lossy(&domain[..domain_len as usize]),
            String::from_utf16_lossy(&name[..name_len as usize])
        ))
    }

    /// Whether the account named `domain_user` is the one this process's
    /// token belongs to, compared by SID. `None` when either SID cannot be
    /// read.
    pub(super) fn same_account(domain_user: &str) -> Option<bool> {
        let token = Token::open()?;
        let buffer = token.read(TokenUser)?;
        if buffer.len() < size_of::<TOKEN_USER>() {
            return None;
        }
        // SAFETY: as in `is_elevated`; the SID lives in `buffer`.
        let user: TOKEN_USER = unsafe { std::ptr::read_unaligned(buffer.as_ptr().cast()) };
        let name: Vec<u16> = domain_user
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        let mut sid = vec![0u8; 256];
        let mut sid_len = u32::try_from(sid.len()).ok()?;
        let mut domain = [0u16; 256];
        let mut domain_len = u32::try_from(domain.len()).ok()?;
        let mut kind = SID_NAME_USE::default();
        // SAFETY: every buffer and length is valid for writing.
        unsafe {
            LookupAccountNameW(
                PCWSTR::null(),
                PCWSTR(name.as_ptr()),
                Some(PSID(sid.as_mut_ptr().cast())),
                &raw mut sid_len,
                Some(PWSTR(domain.as_mut_ptr())),
                &raw mut domain_len,
                &raw mut kind,
            )
        }
        .ok()?;
        // SAFETY: both SIDs are valid for the duration of the call.
        Some(unsafe { EqualSid(user.User.Sid, PSID(sid.as_mut_ptr().cast())) }.is_ok())
    }

    /// The account signed in to this session, as `DOMAIN\user`.
    pub(super) fn session_account() -> Option<String> {
        let user = session_string(WTSUserName)?;
        let domain = session_string(WTSDomainName)?;
        (!user.is_empty()).then(|| format!("{domain}\\{user}"))
    }

    fn session_string(class: WTS_INFO_CLASS) -> Option<String> {
        let mut buffer = PWSTR::null();
        let mut bytes = 0u32;
        // SAFETY: out-pointers are valid; the buffer is freed below.
        unsafe {
            WTSQuerySessionInformationW(
                Some(WTS_CURRENT_SERVER_HANDLE),
                WTS_CURRENT_SESSION,
                class,
                &raw mut buffer,
                &raw mut bytes,
            )
        }
        .ok()?;
        // SAFETY: on success the buffer is a NUL-terminated wide string.
        let text = unsafe { buffer.to_string() }.ok();
        // SAFETY: the buffer came from WTSQuerySessionInformationW.
        unsafe { WTSFreeMemory(buffer.as_ptr().cast()) };
        text
    }

    pub(super) fn start_elevated(executable: &Path) -> Elevation {
        let file: Vec<u16> = executable
            .as_os_str()
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        let parameters: Vec<u16> = ELEVATED_ARG
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        let mut info = SHELLEXECUTEINFOW {
            cbSize: u32::try_from(size_of::<SHELLEXECUTEINFOW>()).unwrap_or_default(),
            lpVerb: w!("runas"),
            lpFile: PCWSTR(file.as_ptr()),
            lpParameters: PCWSTR(parameters.as_ptr()),
            nShow: SW_SHOWNORMAL.0,
            ..Default::default()
        };
        // `ShellExecuteEx` can load shell extensions, which need COM, and
        // `main` runs this before any window has set COM up.
        // SAFETY: no reserved pointer; balanced below only on success.
        let com =
            unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE) };
        // SAFETY: `info` is initialized and its strings outlive the call.
        let launched = unsafe { ShellExecuteExW(&raw mut info) };
        if com.is_ok() {
            // SAFETY: balances the successful CoInitializeEx above.
            unsafe { CoUninitialize() };
        }
        match launched {
            Ok(()) => Elevation::Started,
            Err(error) if error.code() == ERROR_CANCELLED.to_hresult() => Elevation::Declined,
            Err(error) => Elevation::Failed(error.message()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_checks_answer_without_panicking() {
        let _: bool = is_elevated();
        let _ = elevated_as_someone_else();
    }
}
