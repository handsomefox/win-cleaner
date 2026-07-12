//! Recycle Bin integration via `SHFileOperationW` with `FOF_ALLOWUNDO` —
//! the only deletion mechanism in the app. It uses a double-NUL-terminated
//! UTF-16 path list, silent operation, and treats a shell abort as failure.

use std::path::Path;

use cleaner_core::{RecycleError, Recycler};

/// The production [`Recycler`]: moves paths to the Windows Recycle Bin.
/// On non-Windows platforms every call reports `UnsupportedPlatform`.
#[derive(Debug, Clone, Copy, Default)]
pub struct ShellRecycler;

impl Recycler for ShellRecycler {
    fn recycle(&self, paths: &[&Path]) -> Result<(), RecycleError> {
        move_to_recycle_bin(paths)
    }
}

/// Builds the `PCZZWSTR` list `SHFileOperationW` expects: each absolute path
/// encoded as UTF-16 followed by one NUL, the whole list terminated by an
/// extra NUL (`C:\foo\0C:\bar\0\0`). Empty paths are skipped; an empty result
/// means there is nothing to do.
#[cfg_attr(
    not(any(windows, test)),
    expect(
        dead_code,
        reason = "only the Windows recycler calls it; tests cover it everywhere"
    )
)]
fn utf16_path_list(paths: &[&Path]) -> Result<Vec<u16>, RecycleError> {
    let mut list: Vec<u16> = Vec::with_capacity(256);
    for path in paths {
        if path.as_os_str().is_empty() {
            continue;
        }
        let absolute = std::path::absolute(path).map_err(|source| RecycleError::Resolve {
            path: path.to_path_buf(),
            source,
        })?;
        #[cfg(windows)]
        {
            use std::os::windows::ffi::OsStrExt;
            list.extend(absolute.as_os_str().encode_wide());
        }
        #[cfg(not(windows))]
        {
            list.extend(absolute.to_string_lossy().encode_utf16());
        }
        list.push(0);
    }
    if !list.is_empty() {
        // Ensure the final double NUL: each path added one; add one more now.
        list.push(0);
    }
    Ok(list)
}

/// Moves one or more paths to the Recycle Bin in a single shell call.
#[cfg(windows)]
fn move_to_recycle_bin(paths: &[&Path]) -> Result<(), RecycleError> {
    use windows::Win32::UI::Shell::{SHFILEOPSTRUCTW, SHFileOperationW};
    use windows::core::PCWSTR;

    // Numeric values match the Win32 headers; using them directly avoids the
    // crate's mixed constant types.
    const FO_DELETE: u32 = 3;
    const FOF_SILENT: u16 = 0x0004;
    const FOF_NOCONFIRMATION: u16 = 0x0010;
    const FOF_ALLOWUNDO: u16 = 0x0040;
    const FOF_NOERRORUI: u16 = 0x0400;

    if paths.is_empty() {
        return Ok(());
    }
    let list = utf16_path_list(paths)?;
    if list.is_empty() {
        return Ok(());
    }

    let mut op = SHFILEOPSTRUCTW {
        hwnd: windows::Win32::Foundation::HWND::default(),
        wFunc: FO_DELETE,
        pFrom: PCWSTR(list.as_ptr()),
        pTo: PCWSTR::null(),
        fFlags: FOF_ALLOWUNDO | FOF_NOCONFIRMATION | FOF_NOERRORUI | FOF_SILENT,
        fAnyOperationsAborted: windows::core::BOOL(0),
        hNameMappings: std::ptr::null_mut(),
        lpszProgressTitle: PCWSTR::null(),
    };

    // SAFETY: `op` points to a valid SHFILEOPSTRUCTW and `list` (referenced by
    // `pFrom`) is a properly double-NUL-terminated UTF-16 list that outlives
    // the call.
    let ret = unsafe { SHFileOperationW(&raw mut op) };
    if ret != 0 {
        return Err(RecycleError::ShellOperation(ret));
    }
    if op.fAnyOperationsAborted.as_bool() {
        // Treat as a failure to be safe-first.
        return Err(RecycleError::Aborted);
    }
    Ok(())
}

#[cfg(not(windows))]
fn move_to_recycle_bin(paths: &[&Path]) -> Result<(), RecycleError> {
    let _ = paths;
    Err(RecycleError::UnsupportedPlatform)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(windows)]
    use std::fs::File;
    use std::path::MAIN_SEPARATOR_STR;

    fn abs(name: &str) -> std::path::PathBuf {
        std::path::PathBuf::from(format!("{MAIN_SEPARATOR_STR}{name}"))
    }

    #[test]
    fn utf16_list_has_per_path_and_trailing_nuls() {
        let a = abs("foo");
        let b = abs("bar with spaces");
        let list = utf16_path_list(&[&a, &b]).unwrap();

        // One NUL after each path plus the final terminator.
        assert_eq!(list.iter().filter(|&&c| c == 0).count(), 3);
        assert_eq!(&list[list.len() - 2..], &[0, 0]);

        // Decode the first entry back.
        let first_end = list.iter().position(|&c| c == 0).unwrap();
        let first = String::from_utf16(&list[..first_end]).unwrap();
        assert!(first.ends_with("foo"));
    }

    #[test]
    fn utf16_list_skips_empty_paths_and_can_be_empty() {
        let empty = std::path::PathBuf::new();
        let list = utf16_path_list(&[&empty]).unwrap();
        assert!(list.is_empty());
        assert!(utf16_path_list(&[]).unwrap().is_empty());
    }

    #[cfg(not(windows))]
    #[test]
    fn recycler_reports_unsupported_platform() {
        let recycler = ShellRecycler;
        let path = abs("anything");
        let err = recycler.recycle(&[&path]).unwrap_err();
        assert!(matches!(err, RecycleError::UnsupportedPlatform));
    }

    #[cfg(windows)]
    #[test]
    fn recycler_moves_a_real_file_to_the_recycle_bin() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("win-cleaner-recycle-test.tmp");
        File::create(&file).unwrap();

        ShellRecycler.recycle(&[&file]).unwrap();

        assert!(!file.exists());
    }
}
