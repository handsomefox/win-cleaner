//! Model types for the cleanup catalog. The built-in catalog data lives in
//! the `cleaner-catalog` crate; scanning consumes any [`Registry`], which
//! keeps this crate free of app-specific path knowledge.

use std::path::PathBuf;

/// A set of cleanup targets consumed by [`crate::scan::build_plan`].
#[derive(Debug, Clone, Default)]
pub struct Registry {
    pub items: Vec<Item>,
}

/// A folder of `<prefix><build>` version folders beside the `.product.db`
/// that names the installed build, as Blizzard's updater lays them out.
/// Resolved by [`crate::versions::superseded`], which never returns the
/// installed or the newest build.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Versioned {
    pub dir: PathBuf,
    /// The folder name before the build number, such as `Agent.`.
    pub prefix: String,
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
    /// Folders that stay in place while what they hold is recycled, for a
    /// folder Windows or other programs expect to exist, such as `Temp`.
    pub contents: Vec<PathBuf>,
    /// Folders of program versions, resolved at scan time to the superseded
    /// ones and checked again before cleanup.
    pub versioned: Vec<Versioned>,
    /// Pre-selected in the GUI (when non-empty).
    pub default_on: bool,
    /// Executable names of the programs that use these paths, such as
    /// `Discord.exe`. The target is skipped while one runs, when the user
    /// asks for that. Empty for a target that is safe to clean while its
    /// program runs.
    pub processes: Vec<String>,
    /// Moving these paths needs administrator rights.
    pub admin: bool,
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

    /// Builder shorthand: names the programs that use these paths.
    #[must_use]
    pub fn processes<'a>(mut self, names: impl IntoIterator<Item = &'a str>) -> Self {
        self.processes.extend(names.into_iter().map(str::to_owned));
        self
    }

    /// Builder shorthand: appends folders whose contents are recycled while
    /// the folders themselves stay.
    #[must_use]
    pub fn contents(mut self, folders: impl IntoIterator<Item = PathBuf>) -> Self {
        self.contents.extend(folders);
        self
    }

    /// Builder shorthand: appends a folder of `<prefix><build>` program
    /// versions; see [`Versioned`].
    #[must_use]
    pub fn versioned(mut self, dir: PathBuf, prefix: &str) -> Self {
        self.versioned.push(Versioned {
            dir,
            prefix: prefix.to_owned(),
        });
        self
    }
}
