use std::path::PathBuf;

/// Errors surfaced while building a cleanup plan.
#[derive(Debug, thiserror::Error)]
pub enum ScanError {
    #[error("this tool only supports Windows")]
    UnsupportedPlatform,
    #[error("failed to read {path}: {source}")]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
}

/// Errors surfaced while moving paths to the Recycle Bin.
#[derive(Debug, thiserror::Error)]
pub enum RecycleError {
    #[error("recycle bin is only supported on Windows")]
    UnsupportedPlatform,
    #[error("SHFileOperationW failed, code={0}")]
    ShellOperation(i32),
    #[error("operation aborted by shell")]
    Aborted,
    #[error("{0}")]
    Multiple(String),
    #[error("unsafe recycle path: {0}")]
    UnsafePath(PathBuf),
    #[error("failed to resolve {path}: {source}")]
    Resolve {
        path: PathBuf,
        source: std::io::Error,
    },
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
