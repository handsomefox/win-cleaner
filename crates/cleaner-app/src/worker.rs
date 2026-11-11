//! The background worker: a single thread that owns the catalog, roots, and
//! recycler, and talks to the UI thread over channels. The UI never touches
//! the filesystem directly.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use cleaner_catalog::build_registry;
use cleaner_core::{
    ExecResult, FileEntry, Options, Plan, ProgressUpdate, Recycler, Registry, Roots, Rules, Run,
    StoredRun, Target, build_plan, execute_with_result, list_files,
};
use cleaner_platform::ShellRecycler;
use crossbeam_channel::{Receiver, Sender, TryRecvError};
use eframe::egui;

pub(crate) enum Command {
    /// Start a scan. `generation` is echoed back so the UI can ignore events
    /// from a scan it has already abandoned (e.g. after Rescan).
    Scan {
        generation: u64,
        rules: Rules,
    },
    Execute {
        plan: Plan,
        dry_run: bool,
        rules: Rules,
        /// Set by the UI's Stop button; the run ends before its next path.
        stop: Arc<AtomicBool>,
    },
    /// List the files cleaning these targets would move, for the preview.
    /// `request` is echoed back so a stale answer can be told apart.
    ListFiles {
        request: u64,
        targets: Vec<Target>,
        rules: Rules,
    },
    LoadHistory,
    /// Delete one run's stats file, then reload history.
    DeleteRun(PathBuf),
    /// Delete every stats file, then reload history.
    ClearHistory,
}

pub(crate) enum Event {
    ScanProgress {
        generation: u64,
        update: ProgressUpdate,
    },
    ScanDone {
        generation: u64,
        outcome: Result<Plan, String>,
    },
    DeleteProgress(ProgressUpdate),
    DeleteDone {
        result: ExecResult,
        stats_error: Option<String>,
    },
    FilesListed {
        request: u64,
        files: Vec<FileEntry>,
        /// The list stopped at [`FILE_LIST_LIMIT`].
        truncated: bool,
    },
    /// The stored runs and how many files could not be read, or why the
    /// stats directory could not be read.
    HistoryLoaded {
        outcome: Result<(Vec<StoredRun>, usize), String>,
    },
}

/// The worker thread has exited, which only happens when it panicked.
pub(crate) struct WorkerStopped;

pub(crate) struct Worker {
    commands: Sender<Command>,
    events: Receiver<Event>,
}

impl Worker {
    pub(crate) fn send(&self, command: Command) {
        let _ = self.commands.send(command);
    }

    /// The next waiting event, if any. `Err` means the worker thread is gone,
    /// so no event will ever arrive again.
    pub(crate) fn try_recv(&self) -> Result<Option<Event>, WorkerStopped> {
        match self.events.try_recv() {
            Ok(event) => Ok(Some(event)),
            Err(TryRecvError::Empty) => Ok(None),
            Err(TryRecvError::Disconnected) => Err(WorkerStopped),
        }
    }

    #[cfg(test)]
    pub(crate) fn test_channels() -> (Self, Receiver<Command>, Sender<Event>) {
        let (command_tx, command_rx) = crossbeam_channel::unbounded();
        let (event_tx, event_rx) = crossbeam_channel::unbounded();
        (
            Self {
                commands: command_tx,
                events: event_rx,
            },
            command_rx,
            event_tx,
        )
    }
}

/// Spawns the worker thread. It holds a clone of the egui context so every
/// event is followed by a repaint request.
pub(crate) fn spawn(ctx: egui::Context) -> Worker {
    let (command_tx, command_rx) = crossbeam_channel::unbounded::<Command>();
    let (event_tx, event_rx) = crossbeam_channel::unbounded::<Event>();

    std::thread::Builder::new()
        .name("cleaner-worker".to_owned())
        .spawn(move || run(&ctx, &command_rx, &event_tx))
        .expect("failed to spawn worker thread");

    Worker {
        commands: command_tx,
        events: event_rx,
    }
}

fn run(ctx: &egui::Context, commands: &Receiver<Command>, events: &Sender<Event>) {
    let roots = resolve_worker_roots();
    let mut runtime = Runtime {
        registry: build_registry(&roots),
        roots,
        recycler: ShellRecycler,
        stats_dir: cleaner_platform::stats_dir(),
        supported: cfg!(windows) || dev_root().is_some(),
    };

    let emit = |event: Event| {
        let _ = events.send(event);
        ctx.request_repaint();
    };

    run_commands(commands, &mut runtime, emit);
}

struct Runtime<R> {
    roots: Roots,
    registry: Registry,
    recycler: R,
    stats_dir: Option<PathBuf>,
    supported: bool,
}

fn run_commands<R: Recycler>(
    commands: &Receiver<Command>,
    runtime: &mut Runtime<R>,
    mut emit: impl FnMut(Event),
) {
    while let Ok(command) = commands.recv() {
        process_command(runtime, command, &mut emit);
    }
}

#[expect(
    clippy::too_many_lines,
    reason = "one arm per command, each short, in a single match"
)]
fn process_command<R: Recycler>(
    runtime: &mut Runtime<R>,
    command: Command,
    mut emit: impl FnMut(Event),
) {
    let Runtime {
        roots,
        registry,
        recycler,
        stats_dir,
        supported,
    } = runtime;
    match command {
        Command::Scan { generation, rules } => {
            if !*supported || !roots.has_required() {
                emit(Event::ScanDone {
                    generation,
                    outcome: Err(crate::strings::ENGLISH.unsupported_platform.to_owned()),
                });
                return;
            }
            tracing::info!(generation, "scan started");
            let running = app_process::Running::snapshot();
            let plan = build_plan(
                registry,
                roots,
                &rules,
                &|name| running.contains(name),
                |update| {
                    emit(Event::ScanProgress { generation, update });
                },
            );
            tracing::info!(
                generation,
                groups = plan.groups.len(),
                selected = plan.selected,
                total_bytes = plan.total_bytes,
                "scan finished"
            );
            emit(Event::ScanDone {
                generation,
                outcome: Ok(plan),
            });
        }
        Command::Execute {
            plan,
            dry_run,
            rules,
            stop,
        } => {
            tracing::info!(selected = plan.selected, dry_run, "cleanup started");
            let guard_roots = roots.guard_roots();
            let run = Run {
                options: Options { dry_run },
                guard_roots: &guard_roots,
                rules: &rules,
                stop: &stop,
                is_running: &app_process::is_running,
            };
            let result = execute_with_result(&plan, &run, recycler, |update| {
                emit(Event::DeleteProgress(update));
            });
            let mut result = result;
            let stats_error = if dry_run {
                None
            } else {
                write_stats(stats_dir.as_deref(), &mut result)
            };
            tracing::info!(
                total_selected = result.total_selected,
                total_bytes = result.total_bytes,
                freed_bytes = result.freed(),
                error_count = result.error_count,
                stopped = result.stopped,
                "cleanup finished"
            );
            if let Some(err) = &stats_error {
                tracing::warn!("failed to write stats: {err}");
            }
            emit(Event::DeleteDone {
                result,
                stats_error,
            });
        }
        Command::ListFiles {
            request,
            targets,
            rules,
        } => {
            let guard_roots = roots.guard_roots();
            let (files, truncated) = list_target_files(&targets, &rules, &guard_roots);
            emit(Event::FilesListed {
                request,
                files,
                truncated,
            });
        }
        Command::LoadHistory => emit(load_history(stats_dir.as_deref())),
        Command::DeleteRun(path) => {
            if let Err(err) = std::fs::remove_file(&path) {
                tracing::warn!("failed to delete run {}: {err}", path.display());
            }
            emit(load_history(stats_dir.as_deref()));
        }
        Command::ClearHistory => {
            if let Some(dir) = stats_dir.as_deref()
                && let Err(err) = cleaner_core::clear_stats(dir)
            {
                tracing::warn!("failed to clear history: {err}");
            }
            emit(load_history(stats_dir.as_deref()));
        }
    }
}

/// The most files a preview lists. A target holding more says so instead.
pub(crate) const FILE_LIST_LIMIT: usize = 1_000_000;

/// The files cleaning `targets` would move under `rules`, as cleanup plans
/// them. A program version is listed whole, since it is never cleaned in part.
/// A target outside the safe roots, such as one reached through a junction,
/// is never walked, just as cleanup never moves it.
fn list_target_files(
    targets: &[Target],
    rules: &Rules,
    guard_roots: &[&std::path::Path],
) -> (Vec<FileEntry>, bool) {
    let now = std::time::SystemTime::now();
    let whole = Rules::default();
    let mut files = Vec::new();
    for target in targets {
        let remaining = FILE_LIST_LIMIT.saturating_sub(files.len());
        if remaining == 0 {
            return (files, true);
        }
        if !cleaner_core::is_safe_path(&target.path, guard_roots) {
            continue;
        }
        if target.version_rule.is_some() {
            if !rules.is_excluded(&target.path) && !rules.has_exclusion_inside(&target.path) {
                files.extend(list_files(&target.path, false, &whole, now, remaining));
            }
        } else {
            files.extend(list_files(
                &target.path,
                target.keep_folder,
                rules,
                now,
                remaining,
            ));
        }
    }
    let truncated = files.len() >= FILE_LIST_LIMIT;
    (files, truncated)
}

/// Loads the stats directory into a [`Event::HistoryLoaded`], most recent
/// cleanup first.
fn load_history(stats_dir: Option<&std::path::Path>) -> Event {
    let Some(dir) = stats_dir else {
        return Event::HistoryLoaded {
            outcome: Err(crate::strings::ENGLISH.stats_dir_unavailable.to_owned()),
        };
    };
    let outcome = cleaner_core::load_stats(dir)
        .map(|(mut runs, skipped)| {
            runs.sort_by_key(|run| std::cmp::Reverse(run.result.run_timestamp()));
            (runs, skipped)
        })
        .map_err(|err| err.to_string());
    Event::HistoryLoaded { outcome }
}

fn write_stats(stats_dir: Option<&std::path::Path>, result: &mut ExecResult) -> Option<String> {
    let Some(dir) = stats_dir else {
        return Some(crate::strings::ENGLISH.stats_dir_unavailable.to_owned());
    };
    match cleaner_core::write_stats(dir, result) {
        Ok(path) => {
            tracing::info!("stats written to {}", path.display());
            None
        }
        Err(err) => Some(err.to_string()),
    }
}

fn resolve_worker_roots() -> Roots {
    if let Some(root) = dev_root() {
        return dev_roots(&root);
    }
    cleaner_platform::resolve_roots()
}

/// Debug-only escape hatch so the full UI flow can be exercised on Linux:
/// point `WIN_CLEANER_DEV_ROOT` at a scratch directory laid out like a user
/// profile. Release builds ignore it.
fn dev_root() -> Option<PathBuf> {
    if cfg!(debug_assertions) {
        std::env::var_os("WIN_CLEANER_DEV_ROOT").map(PathBuf::from)
    } else {
        None
    }
}

fn dev_roots(root: &std::path::Path) -> Roots {
    Roots {
        // One component per `join`: the Windows shell rejects a path that
        // mixes `/` into its `\` separators.
        local_app_data: Some(root.join("AppData").join("Local")),
        roaming_app_data: Some(root.join("AppData").join("Roaming")),
        program_data: Some(root.join("ProgramData")),
        user_profile: Some(root.to_path_buf()),
        ..Roots::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cleaner_core::{Group, RecycleError};
    use jiff::Timestamp;
    use std::cell::RefCell;
    use std::fs;
    use std::path::Path;

    #[derive(Default)]
    struct MockRecycler {
        calls: RefCell<Vec<PathBuf>>,
    }

    impl Recycler for MockRecycler {
        fn recycle(&self, paths: &[&Path]) -> Result<(), RecycleError> {
            self.calls
                .borrow_mut()
                .extend(paths.iter().map(|path| path.to_path_buf()));
            for path in paths {
                if path.is_dir() {
                    fs::remove_dir_all(path).unwrap();
                } else {
                    fs::remove_file(path).unwrap();
                }
            }
            Ok(())
        }
    }

    fn roots(base: &Path) -> Roots {
        Roots {
            local_app_data: Some(base.join("Local")),
            roaming_app_data: Some(base.join("Roaming")),
            program_data: Some(base.join("ProgramData")),
            user_profile: Some(base.join("Profile")),
            ..Roots::default()
        }
    }

    fn runtime(base: &Path) -> Runtime<MockRecycler> {
        let roots = roots(base);
        Runtime {
            registry: Registry {
                items: vec![
                    cleaner_core::Item::new("App", "cache", true).paths([roots
                        .local_app_data
                        .as_ref()
                        .unwrap()
                        .join("App/Cache")]),
                ],
            },
            roots,
            recycler: MockRecycler::default(),
            stats_dir: Some(base.join("stats")),
            supported: true,
        }
    }

    #[test]
    fn scan_emits_progress_and_completion_with_generation() {
        let dir = tempfile::tempdir().unwrap();
        let cache = dir.path().join("Local/App/Cache");
        fs::create_dir_all(&cache).unwrap();
        fs::write(cache.join("data.bin"), [0_u8; 7]).unwrap();
        let mut runtime = runtime(dir.path());
        let mut events = Vec::new();

        process_command(
            &mut runtime,
            Command::Scan {
                generation: 9,
                rules: Rules::default(),
            },
            |event| {
                events.push(event);
            },
        );

        assert!(matches!(
            events[0],
            Event::ScanProgress { generation: 9, .. }
        ));
        let Event::ScanDone {
            generation,
            outcome: Ok(plan),
        } = events.pop().unwrap()
        else {
            panic!("expected completed scan");
        };
        assert_eq!(generation, 9);
        assert_eq!(plan.total_bytes, 7);
    }

    #[test]
    fn scan_rejects_unsupported_or_incomplete_roots() {
        let dir = tempfile::tempdir().unwrap();
        let mut runtime = runtime(dir.path());
        runtime.supported = false;
        let mut events = Vec::new();
        process_command(
            &mut runtime,
            Command::Scan {
                generation: 1,
                rules: Rules::default(),
            },
            |event| {
                events.push(event);
            },
        );
        assert!(matches!(
            events.as_slice(),
            [Event::ScanDone {
                outcome: Err(_),
                ..
            }]
        ));

        runtime.supported = true;
        runtime.roots = Roots::default();
        events.clear();
        process_command(
            &mut runtime,
            Command::Scan {
                generation: 2,
                rules: Rules::default(),
            },
            |event| {
                events.push(event);
            },
        );
        assert!(matches!(
            events.as_slice(),
            [Event::ScanDone {
                outcome: Err(_),
                ..
            }]
        ));
    }

    #[test]
    fn execution_reports_progress_deletion_dry_run_and_stats_failures() {
        let dir = tempfile::tempdir().unwrap();
        let mut runtime = runtime(dir.path());
        let cache = dir.path().join("Local/App/Cache");
        fs::create_dir_all(&cache).unwrap();
        fs::write(cache.join("data.bin"), [0_u8; 3]).unwrap();
        let mut plan = Plan {
            groups: vec![Group {
                targets: vec![Target::whole(cache.clone(), 3)],
                bytes: 3,
                on: true,
                ..Group::new("App", "cache")
            }],
            ..Plan::default()
        };
        plan.recompute_totals();

        let mut events = Vec::new();
        process_command(
            &mut runtime,
            Command::Execute {
                plan: plan.clone(),
                dry_run: true,
                rules: Rules::default(),
                stop: Arc::new(AtomicBool::new(false)),
            },
            |event| events.push(event),
        );
        assert!(cache.exists());
        assert!(matches!(
            events.as_slice(),
            [Event::DeleteDone {
                stats_error: None,
                ..
            }]
        ));

        fs::write(dir.path().join("blocked-stats"), b"file").unwrap();
        runtime.stats_dir = Some(dir.path().join("blocked-stats/child"));
        events.clear();
        process_command(
            &mut runtime,
            Command::Execute {
                plan,
                dry_run: false,
                rules: Rules::default(),
                stop: Arc::new(AtomicBool::new(false)),
            },
            |event| events.push(event),
        );
        assert!(!cache.exists());
        assert!(matches!(events.first(), Some(Event::DeleteProgress(_))));
        assert!(matches!(
            events.last(),
            Some(Event::DeleteDone {
                stats_error: Some(_),
                ..
            })
        ));
    }

    fn write_run(dir: &Path, timestamp: &str) -> PathBuf {
        let mut result = ExecResult::begin(&Plan::default(), Options::default());
        result.finished_at = timestamp.parse::<Timestamp>().unwrap();
        cleaner_core::write_stats(dir, &mut result).unwrap()
    }

    #[test]
    fn history_loads_newest_first_then_deletes_and_clears() {
        let dir = tempfile::tempdir().unwrap();
        let mut runtime = runtime(dir.path());
        let stats = runtime.stats_dir.clone().unwrap();
        fs::create_dir_all(&stats).unwrap();
        let older = write_run(&stats, "2026-01-01T00:00:00Z");
        let newer = write_run(&stats, "2026-02-01T00:00:00Z");
        fs::write(stats.join("broken.json"), b"nope").unwrap();

        let mut events = Vec::new();
        process_command(&mut runtime, Command::LoadHistory, |event| {
            events.push(event);
        });
        let Event::HistoryLoaded {
            outcome: Ok((runs, skipped)),
        } = events.pop().unwrap()
        else {
            panic!("expected history");
        };
        assert_eq!(runs[0].path, newer);
        assert_eq!(skipped, 1);

        process_command(&mut runtime, Command::DeleteRun(older.clone()), |_| {});
        assert!(!older.exists());
        process_command(&mut runtime, Command::ClearHistory, |_| {});
        assert!(!newer.exists());
        assert!(!stats.join("broken.json").exists());
    }

    #[test]
    fn unavailable_history_and_disconnected_channel_shutdown_cleanly() {
        let dir = tempfile::tempdir().unwrap();
        let mut runtime = runtime(dir.path());
        runtime.stats_dir = None;
        assert!(matches!(
            load_history(None),
            Event::HistoryLoaded { outcome: Err(_) }
        ));
        let (tx, rx) = crossbeam_channel::unbounded();
        drop(tx);
        run_commands(&rx, &mut runtime, |_| panic!("no events expected"));
    }

    #[test]
    fn the_file_list_walks_only_targets_inside_the_safe_roots() {
        let dir = tempfile::tempdir().unwrap();
        let mut runtime = runtime(dir.path());
        let inside = dir.path().join("Local/App/Cache");
        let outside = dir.path().join("Elsewhere/Cache");
        for folder in [&inside, &outside] {
            fs::create_dir_all(folder).unwrap();
            fs::write(folder.join("file"), b"data").unwrap();
        }
        let mut events = Vec::new();
        process_command(
            &mut runtime,
            Command::ListFiles {
                request: 1,
                targets: vec![
                    Target::whole(inside.clone(), 4),
                    Target::whole(outside.clone(), 4),
                ],
                rules: Rules::default(),
            },
            |event| events.push(event),
        );
        let Some(Event::FilesListed { files, .. }) = events.pop() else {
            panic!("expected the file list");
        };
        assert_eq!(files.len(), 1);
        assert!(files[0].path.starts_with(&inside));
    }
}
