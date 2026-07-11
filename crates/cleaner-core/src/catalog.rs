//! Model types for the cleanup catalog. The built-in catalog data lives in
//! the `cleaner-catalog` crate; scanning consumes any [`Registry`], which
//! keeps this crate free of app-specific path knowledge.

use std::path::PathBuf;

/// A set of cleanup targets consumed by [`crate::scan::build_plan`].
#[derive(Debug, Clone, Default)]
pub struct Registry {
    pub items: Vec<Item>,
}

/// A single cleanup target under an app name. Each item is individually
/// selectable in the GUI.
#[derive(Debug, Clone, Default)]
pub struct Item {
    pub app: String,
    /// Human-friendly description shown in the UI.
    pub label: String,
    /// Static paths (existence checked at scan time).
    pub paths: Vec<PathBuf>,
    /// Glob patterns expanded at scan time.
    pub globs: Vec<PathBuf>,
    /// Pre-selected in the GUI (when non-empty).
    pub default_on: bool,
}

impl Item {
    #[must_use]
    pub fn new(app: &str, label: &str, default_on: bool) -> Self {
        Self {
            app: app.to_owned(),
            label: label.to_owned(),
            default_on,
            ..Self::default()
        }
    }

    /// Builder shorthand: appends static paths.
    #[must_use]
    pub fn paths(mut self, paths: impl IntoIterator<Item = PathBuf>) -> Self {
        self.paths.extend(paths);
        self
    }

    /// Builder shorthand: appends glob patterns.
    #[must_use]
    pub fn globs(mut self, globs: impl IntoIterator<Item = PathBuf>) -> Self {
        self.globs.extend(globs);
        self
    }
}
