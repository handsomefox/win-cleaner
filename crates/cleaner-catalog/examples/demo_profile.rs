//! Builds a user profile to take screenshots against.
//!
//! A real profile is someone's own PC, so the pictures in the readme are taken against this
//! instead. It fills the cleanup targets of a few common apps and leaves the rest of the catalog
//! empty, the way a real profile looks. Files are written sparse: the sizes the window reports
//! are the sizes on disk, but the directory costs a few kilobytes rather than the gigabytes it
//! claims.
//!
//! The layout is the one `WIN_CLEANER_DEV_ROOT` expects, which only a debug build reads:
//!
//! ```
//! cargo run -p cleaner-catalog --example demo_profile -- /tmp/demo
//! WIN_CLEANER_DEV_ROOT=/tmp/demo cargo run -p cleaner-app
//! ```

#![expect(
    clippy::print_stdout,
    clippy::print_stderr,
    reason = "a developer tool reports what it did"
)]

use cleaner_catalog::build_registry;
use cleaner_core::Roots;
use std::fs::{self, File};
use std::hash::{DefaultHasher, Hash as _, Hasher as _};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

/// The apps that get data. Every other app in the catalog stays absent.
const APPS: &[&str] = &[
    "Chrome",
    "Edge",
    "Firefox",
    "Discord",
    "Spotify",
    "Steam",
    "Epic Games Launcher",
    "NVIDIA",
    "OBS Studio",
    "VSCode",
    "npm",
    "pip",
    "Telegram",
    "Windows",
];

/// Extensions that mark a target as a file rather than a folder.
const FILE_EXTENSIONS: &[&str] = &["bin", "db", "dmp", "json", "log", "nupkg", "tgz"];

const MIB: u64 = 1024 * 1024;

fn main() -> ExitCode {
    let Some(root) = std::env::args_os().nth(1).map(PathBuf::from) else {
        eprintln!("usage: demo_profile <directory>");
        return ExitCode::from(2);
    };
    match build(&root) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("demo_profile: {err}");
            ExitCode::FAILURE
        }
    }
}

fn build(root: &Path) -> std::io::Result<()> {
    if root.exists() {
        return Err(std::io::Error::other(format!(
            "{} already exists; pick a new directory",
            root.display()
        )));
    }
    let roots = Roots {
        local_app_data: Some(root.join("AppData/Local")),
        roaming_app_data: Some(root.join("AppData/Roaming")),
        program_data: Some(root.join("ProgramData")),
        user_profile: Some(root.to_path_buf()),
        ..Roots::default()
    };
    let registry = build_registry(&roots);
    let mut targets = 0;
    let mut bytes = 0;
    for item in registry
        .items
        .iter()
        .filter(|item| APPS.contains(&item.app.as_str()))
    {
        let globbed = item.globs.iter().map(|pattern| expand(pattern));
        for path in item.paths.iter().cloned().chain(globbed) {
            bytes += fill(&path)?;
            targets += 1;
        }
    }
    println!(
        "Wrote {targets} paths holding {} MiB under {}",
        bytes / MIB,
        root.display()
    );
    Ok(())
}

/// Turns a glob into one path it would match: a bare `*` becomes a profile
/// named `Default`, and a `*` inside a name becomes `1`.
fn expand(pattern: &Path) -> PathBuf {
    pattern
        .components()
        .map(|part| {
            let part = part.as_os_str().to_string_lossy();
            if part == "*" {
                "Default".to_owned()
            } else {
                part.replace('*', "1")
            }
        })
        .collect()
}

/// Writes a sparse file at `path`, or three inside it when it names a folder,
/// and returns how many bytes they claim. Sizes come from a hash of the path,
/// so the same directory argument always produces the same picture.
fn fill(path: &Path) -> std::io::Result<u64> {
    let mut hasher = DefaultHasher::new();
    path.hash(&mut hasher);
    let seed = hasher.finish();
    // Squaring skews the sizes the way real caches are: mostly small, a few large.
    let size = (seed % 48 + 1).pow(2) * MIB / 6;
    let is_file = path
        .extension()
        .is_some_and(|ext| FILE_EXTENSIONS.contains(&ext.to_string_lossy().as_ref()));
    let files = if is_file {
        vec![(path.to_path_buf(), size)]
    } else {
        ["data_0", "data_1", "f_000001"]
            .iter()
            .zip([size / 2, size / 3, size - size / 2 - size / 3])
            .map(|(name, part)| (path.join(name), part))
            .collect()
    };
    for (file, part) in &files {
        if let Some(parent) = file.parent() {
            fs::create_dir_all(parent)?;
        }
        File::create(file)?.set_len(*part)?;
    }
    Ok(size)
}
