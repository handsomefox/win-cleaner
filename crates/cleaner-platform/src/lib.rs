//! Windows platform integration for win-cleaner.
//!
//! This crate is the only place allowed to talk to Windows APIs. On other
//! platforms it compiles to stubs so the workspace builds and tests run on
//! Linux; the stubs report `UnsupportedPlatform` errors at runtime.

pub mod recycle;
pub mod roots;

pub use recycle::ShellRecycler;
pub use roots::{logs_dir, resolve_roots, stats_dir};
