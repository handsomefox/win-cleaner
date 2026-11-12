//! Running with administrator rights.
//!
//! The app starts unelevated (`asInvoker`). Targets under Program Files and
//! Windows need administrator rights, so the user can start a second copy
//! elevated. UAC can elevate as a different account when the signed-in one
//! is not an administrator, and that copy would resolve another account's
//! user folders, so [`account_check`] lets the app refuse to clean there.

use std::path::Path;

/// The argument an elevated copy is started with, so it never asks for
/// elevation again even when the setting to always start elevated is on.
pub const ELEVATED_ARG: &str = "--elevated";

/// Starts the argument that names the process an elevated copy waits for:
/// `--after=<pid>`.
pub const AFTER_ARG: &str = "--after=";

/// Whether this process runs with administrator rights.
#[must_use]
pub fn is_elevated() -> bool {
    #[cfg(windows)]
    {
        imp::elevation().unwrap_or(false)
    }
    #[cfg(not(windows))]
    {
        false
    }
}

/// Which account this process runs as, next to the one signed in.
#[derive(Debug, PartialEq, Eq)]
pub enum Account {
    /// Not elevated, or elevated as the signed-in account.
    Same,
    /// Elevated as another account: this process's name, then the
    /// session's, both as `DOMAIN\user`.
    Different { process: String, session: String },
    /// Elevated, and the two accounts could not be compared.
    Unknown,
}

/// Compares the account this process runs as with the one signed in to the
/// session, by SID, since two APIs can spell one account's name differently.
///
/// An unelevated process is always [`Account::Same`]: it runs as whoever
/// started it. An elevated one is [`Account::Same`] only when the SIDs are
/// read and match, so an answer that cannot be established blocks cleaning.
/// A process whose elevation cannot be read is compared too.
#[must_use]
pub fn account_check() -> Account {
    #[cfg(windows)]
    {
        if imp::elevation() == Some(false) {
            return Account::Same;
        }
        let process = imp::process_account();
        let session = imp::session_account();
        tracing::info!(?process, ?session, "elevated; checking the account");
        let (Some(process), Some(session)) = (process, session) else {
            tracing::warn!("could not read both account names");
            return Account::Unknown;
        };
        match imp::same_account(&session) {
            Some(true) => Account::Same,
            Some(false) => Account::Different { process, session },
            None => {
                tracing::warn!("could not compare the accounts");
                Account::Unknown
            }
        }
    }
    #[cfg(not(windows))]
    {
        Account::Same
    }
}

/// Waits for the process with id `pid` to exit, however long that takes,
/// when it runs `executable`. Returns at once when it has already exited,
/// cannot be opened, or runs another program, which happens when Windows
/// has given the id to a new process.
///
/// An elevated copy calls this before it loads its settings, because the
/// window that started it finishes writing them only as it exits.
pub fn wait_for_exit(pid: u32, executable: &Path) {
    #[cfg(windows)]
    {
        imp::wait_for_exit(pid, executable);
    }
    #[cfg(not(windows))]
    {
        let _ = (pid, executable);
    }
}

/// The process id an elevated copy was told to wait for, from `args`.
#[must_use]
pub fn after_pid(args: impl IntoIterator<Item = String>) -> Option<u32> {
    args.into_iter()
        .find_map(|arg| arg.strip_prefix(AFTER_ARG)?.parse().ok())
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
/// passing [`ELEVATED_ARG`]. With `after`, the copy also gets [`AFTER_ARG`]
/// with this process's id, and waits for this process to exit before it
/// reads its settings.
#[must_use]
pub fn start_elevated(executable: &Path, after: bool) -> Elevation {
    let mut parameters = ELEVATED_ARG.to_owned();
    if after {
        parameters.push(' ');
        parameters.push_str(AFTER_ARG);
        parameters.push_str(&std::process::id().to_string());
    }
    #[cfg(windows)]
    {
        imp::start_elevated(executable, &parameters)
    }
    #[cfg(not(windows))]
    {
        let _ = (executable, parameters);
        Elevation::Failed("administrator rights are only available on Windows".to_owned())
    }
}

#[cfg(windows)]
mod imp {
    use std::ffi::c_void;
    use std::os::windows::ffi::OsStrExt as _;
    use std::path::Path;

    use windows::Win32::Foundation::WAIT_TIMEOUT;
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
    use windows::Win32::System::Threading::{
        GetCurrentProcess, OpenProcess, OpenProcessToken, PROCESS_NAME_WIN32,
        PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_SYNCHRONIZE, QueryFullProcessImageNameW,
        WaitForSingleObject,
    };
    use windows::Win32::UI::Shell::{SHELLEXECUTEINFOW, ShellExecuteExW};
    use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
    use windows::core::{PCWSTR, PWSTR, w};

    use super::Elevation;

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

    /// Whether this process's token is elevated, or `None` when the token
    /// cannot be read.
    pub(super) fn elevation() -> Option<bool> {
        let token = Token::open()?;
        let buffer = token.read(TokenElevation)?;
        if buffer.len() < size_of::<TOKEN_ELEVATION>() {
            return None;
        }
        // SAFETY: the buffer holds at least one TOKEN_ELEVATION, written by
        // GetTokenInformation; read_unaligned copes with the Vec's alignment.
        let elevation: TOKEN_ELEVATION =
            unsafe { std::ptr::read_unaligned(buffer.as_ptr().cast()) };
        Some(elevation.TokenIsElevated != 0)
    }

    /// The account this process runs as, as `DOMAIN\user`.
    pub(super) fn process_account() -> Option<String> {
        let token = Token::open()?;
        let buffer = token.read(TokenUser)?;
        if buffer.len() < size_of::<TOKEN_USER>() {
            return None;
        }
        // SAFETY: as in `elevation`; the SID it points to lives in `buffer`,
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
        // SAFETY: as in `elevation`; the SID lives in `buffer`.
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

    pub(super) fn wait_for_exit(pid: u32, executable: &Path) {
        // SAFETY: plain call; a failure means there is nothing to wait for.
        let Ok(process) = (unsafe {
            OpenProcess(
                PROCESS_SYNCHRONIZE | PROCESS_QUERY_LIMITED_INFORMATION,
                false,
                pid,
            )
        }) else {
            return;
        };
        if runs(process, executable) {
            loop {
                // SAFETY: `process` is a valid handle with SYNCHRONIZE access.
                let waited = unsafe { WaitForSingleObject(process, 15_000) };
                if waited != WAIT_TIMEOUT {
                    break;
                }
                tracing::info!(pid, "still waiting for the window that started this copy");
            }
        }
        // SAFETY: the handle came from OpenProcess and is closed once.
        let _ = unsafe { CloseHandle(process) };
    }

    /// Whether `process` runs the program at `executable`.
    fn runs(process: HANDLE, executable: &Path) -> bool {
        let mut buffer = [0u16; 1024];
        let mut length = u32::try_from(buffer.len()).unwrap_or_default();
        // SAFETY: `buffer` holds `length` writable characters.
        let named = unsafe {
            QueryFullProcessImageNameW(
                process,
                PROCESS_NAME_WIN32,
                PWSTR(buffer.as_mut_ptr()),
                &raw mut length,
            )
        };
        if named.is_err() {
            return false;
        }
        let image = String::from_utf16_lossy(&buffer[..length as usize]);
        image.eq_ignore_ascii_case(&executable.to_string_lossy())
    }

    pub(super) fn start_elevated(executable: &Path, parameters: &str) -> Elevation {
        let file: Vec<u16> = executable
            .as_os_str()
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        let parameters: Vec<u16> = parameters
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
        let _ = account_check();
    }

    #[test]
    fn the_process_to_wait_for_comes_from_its_argument() {
        let args = |list: &[&str]| list.iter().map(|&arg| arg.to_owned()).collect::<Vec<_>>();
        assert_eq!(
            after_pid(args(&["app.exe", "--elevated", "--after=4242"])),
            Some(4242)
        );
        assert_eq!(after_pid(args(&["app.exe", "--elevated"])), None);
        assert_eq!(after_pid(args(&["app.exe", "--after=nope"])), None);
    }
}
