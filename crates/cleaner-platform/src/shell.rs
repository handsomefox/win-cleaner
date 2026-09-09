//! Asking the system file manager to open a path.

use std::path::Path;

/// Values at or below this mean `ShellExecuteW` failed. Above it, the return
/// value is a legacy instance handle.
#[cfg(windows)]
const SHELL_EXECUTE_MIN_SUCCESS: usize = 32;

/// Opens a folder in the system file manager.
///
/// Spawning `explorer.exe` as a child process is unreliable: Explorer hands
/// the path to the running shell and exits, so the launch can fail after
/// `spawn` has already reported success. `ShellExecuteW` is the documented way
/// to ask the shell to open a path, and it starts no process of ours.
///
/// # Errors
/// Returns a message describing why the folder could not be opened.
#[cfg(windows)]
pub fn open_folder(directory: &Path) -> Result<(), String> {
    use std::os::windows::ffi::OsStrExt as _;
    use windows::Win32::UI::Shell::ShellExecuteW;
    use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
    use windows::core::PCWSTR;

    let mut path: Vec<u16> = directory.as_os_str().encode_wide().collect();
    if path.contains(&0) {
        return Err("the folder path contains NUL".to_owned());
    }
    path.push(0);

    // SAFETY: `path` is NUL-terminated and outlives this synchronous call, the
    // verb is a static wide literal, and every remaining argument is the
    // documented null. egui runs this on the window thread, where winit has
    // already initialized COM for the shell extensions Explorer loads.
    let instance = unsafe {
        ShellExecuteW(
            None,
            windows::core::w!("open"),
            PCWSTR(path.as_ptr()),
            PCWSTR::null(),
            PCWSTR::null(),
            SW_SHOWNORMAL,
        )
    };
    if instance.0.addr() <= SHELL_EXECUTE_MIN_SUCCESS {
        return Err(format!(
            "the shell refused to open the folder (code {})",
            instance.0.addr()
        ));
    }
    Ok(())
}

/// Opens a folder in the system file manager.
///
/// # Errors
/// Returns a message describing why the folder could not be opened.
#[cfg(not(windows))]
pub fn open_folder(directory: &Path) -> Result<(), String> {
    let opener = if cfg!(target_os = "macos") {
        "open"
    } else {
        "xdg-open"
    };
    std::process::Command::new(opener)
        .arg(directory)
        .spawn()
        .map(|_| ())
        .map_err(|error| error.to_string())
}
