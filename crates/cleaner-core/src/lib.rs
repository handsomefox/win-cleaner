//! Portable domain logic for win-cleaner.
//!
//! Everything in this crate is platform-neutral and testable on any OS: the
//! cleanup catalog, plan building, safety guards, execution strategy, and run
//! statistics. All filesystem roots are injected via [`Roots`] rather than
//! read from the environment, and deletion goes through the [`Recycler`]
//! trait so the Windows Recycle Bin implementation lives in
//! `cleaner-platform`.

pub mod catalog;
pub mod empty_folders;
pub mod error;
pub mod execute;
pub mod format;
pub mod globs;
pub mod plan;
pub mod roots;
pub mod safety;
pub mod scan;
pub mod stats;

pub use catalog::{Item, Registry};
pub use empty_folders::EMPTY_FOLDERS_APP;
pub use error::{RecycleError, ScanError, StatsError};
pub use execute::{Recycler, execute_with_result};
pub use format::human_bytes;
pub use plan::{AppGroup, Group, Options, Phase, Plan, ProgressUpdate};
pub use roots::Roots;
pub use safety::is_safe_path;
pub use scan::build_plan;
pub use stats::{
    ExecResult, GroupResult, PathError, StoredRun, clear_stats, load_stats, write_stats,
};
