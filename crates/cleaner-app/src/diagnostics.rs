//! File-based diagnostics, modeled on dlss-updater: every run appends to a
//! dated log in `%LOCALAPPDATA%\win-cleaner\logs` that the user can share
//! when something goes wrong. Panics are logged before the process dies.

use std::fs::{self, File};
use std::io::Write as _;
use std::path::PathBuf;
use std::sync::Mutex;

use tracing_subscriber::EnvFilter;

/// Keep about a week of daily log files.
const MAX_LOG_FILES: usize = 7;

/// Installs the tracing subscriber and panic hook. Returns the log file path
/// when file logging could be set up; logging falls back to stderr otherwise.
pub(crate) fn init() -> Option<PathBuf> {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    let log_path = open_log_file();
    match log_path
        .as_ref()
        .and_then(|path| File::options().create(true).append(true).open(path).ok())
    {
        Some(file) => {
            tracing_subscriber::fmt()
                .with_env_filter(filter)
                .with_ansi(false)
                .with_writer(Mutex::new(file))
                .init();
        }
        None => {
            tracing_subscriber::fmt().with_env_filter(filter).init();
        }
    }

    install_panic_hook();
    log_path
}

/// Chooses today's log file inside the logs directory, pruning old files so
/// the directory never grows past [`MAX_LOG_FILES`].
fn open_log_file() -> Option<PathBuf> {
    let dir = cleaner_platform::logs_dir()?;
    let today = jiff::Zoned::now().strftime("%Y-%m-%d").to_string();
    prepare_log_file(&dir, &today)
}

fn prepare_log_file(dir: &std::path::Path, date: &str) -> Option<PathBuf> {
    fs::create_dir_all(dir).ok()?;
    prune_old_logs(dir);
    Some(dir.join(format!("win-cleaner-{date}.log")))
}

fn prune_old_logs(dir: &std::path::Path) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    let mut logs: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| {
                    name.starts_with("win-cleaner-")
                        && std::path::Path::new(name)
                            .extension()
                            .is_some_and(|ext| ext.eq_ignore_ascii_case("log"))
                })
        })
        .collect();
    if logs.len() < MAX_LOG_FILES {
        return;
    }
    // Dated names sort chronologically; drop the oldest beyond the budget
    // (today's file is about to be added).
    logs.sort();
    let excess = logs.len() + 1 - MAX_LOG_FILES;
    for path in logs.into_iter().take(excess) {
        let _ = fs::remove_file(path);
    }
}

fn install_panic_hook() {
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let backtrace = std::backtrace::Backtrace::force_capture();
        tracing::error!("panic: {info}\n{backtrace}");
        // The subscriber writes through a mutex-guarded file; make a best
        // effort to get the message out before the process dies.
        let _ = std::io::stderr().flush();
        default_hook(info);
    }));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selects_dated_log_and_prunes_only_old_matching_files() {
        let dir = tempfile::tempdir().unwrap();
        for day in 1..=9 {
            fs::write(
                dir.path().join(format!("win-cleaner-2026-07-{day:02}.log")),
                b"log",
            )
            .unwrap();
        }
        fs::write(dir.path().join("keep.txt"), b"keep").unwrap();
        fs::write(dir.path().join("other.log"), b"keep").unwrap();

        let selected = prepare_log_file(dir.path(), "2026-07-10").unwrap();
        assert_eq!(selected, dir.path().join("win-cleaner-2026-07-10.log"));
        let retained = fs::read_dir(dir.path())
            .unwrap()
            .filter_map(Result::ok)
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .filter(|name| {
                name.starts_with("win-cleaner-")
                    && std::path::Path::new(name)
                        .extension()
                        .is_some_and(|ext| ext.eq_ignore_ascii_case("log"))
            })
            .collect::<Vec<_>>();
        assert_eq!(retained.len(), MAX_LOG_FILES - 1);
        assert!(
            retained
                .iter()
                .all(|name| name.as_str() >= "win-cleaner-2026-07-04.log")
        );
        assert!(dir.path().join("keep.txt").exists());
        assert!(dir.path().join("other.log").exists());
    }

    #[test]
    fn filesystem_errors_are_non_fatal() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("not-a-directory");
        fs::write(&file, b"file").unwrap();
        prune_old_logs(&file);
        assert!(prepare_log_file(&file, "2026-07-10").is_none());
    }
}
