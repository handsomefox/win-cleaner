use std::path::PathBuf;

/// Errors surfaced while moving paths to the Recycle Bin.
#[derive(Debug, thiserror::Error)]
pub enum RecycleError {
    #[error("recycle bin is only supported on Windows")]
    UnsupportedPlatform,
    /// The shell failed with this `HRESULT`. `message` is the system's text
    /// for it, empty when Windows has none.
    #[error("Recycle Bin operation failed (0x{:08X}){}", code.cast_unsigned(), message_suffix(message))]
    ShellOperation { code: i32, message: String },
    #[error("operation aborted by shell")]
    Aborted,
    /// The shell deleted the item for good instead of moving it to the
    /// Recycle Bin, such as when the Recycle Bin is turned off for its drive.
    #[error("permanently deleted, not in the Recycle Bin")]
    NotRecycled,
    /// The shell would have deleted the item for good, because the Recycle
    /// Bin is turned off or too small for its drive, so it was left alone.
    #[error(
        "not moved: the Recycle Bin is turned off or too small for this drive, so Windows would \
         have deleted it for good. Nothing was deleted."
    )]
    RecycleBinOff,
    /// A folder above the path could not be opened and held in place for the
    /// move, so nothing was moved.
    #[error("could not hold the folders above {path} in place: {source}")]
    ParentNotHeld {
        path: PathBuf,
        source: std::io::Error,
    },
    /// The path's metadata could not be read, so nothing says what it is.
    #[error("could not inspect {path}: {source}")]
    Uninspectable {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("{0}")]
    Multiple(String),
    #[error("unsafe recycle path: {0}")]
    UnsafePath(PathBuf),
}

fn message_suffix(message: &str) -> String {
    let message = message.trim();
    if message.is_empty() {
        String::new()
    } else {
        format!(": {message}")
    }
}

/// Errors surfaced while reading or writing run statistics.
#[derive(Debug, thiserror::Error)]
pub enum StatsError {
    #[error("failed to access stats directory {path}: {source}")]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("failed to encode stats: {0}")]
    Encode(#[from] serde_json::Error),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shell_errors_show_the_code_in_hex_with_the_message() {
        let error = RecycleError::ShellOperation {
            code: 0x8007_0005_u32.cast_signed(),
            message: "Access is denied.\r\n".to_owned(),
        };
        assert_eq!(
            error.to_string(),
            "Recycle Bin operation failed (0x80070005): Access is denied."
        );
        let bare = RecycleError::ShellOperation {
            code: 5,
            message: String::new(),
        };
        assert_eq!(
            bare.to_string(),
            "Recycle Bin operation failed (0x00000005)"
        );
    }
}
