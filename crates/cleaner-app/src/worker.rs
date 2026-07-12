//! The background worker: a single thread that owns the catalog, roots, and
//! recycler, and talks to the UI thread over channels. The UI never touches
//! the filesystem directly.

use std::path::PathBuf;

use cleaner_catalog::build_registry;
use cleaner_core::{
    ExecResult, Options, Plan, ProgressUpdate, Recycler, Registry, Roots, StoredRun, build_plan,
    execute_with_result,
};
use cleaner_platform::ShellRecycler;
use crossbeam_channel::{Receiver, Sender};
use eframe::egui;

pub(crate) enum Command {
    /// Start a scan. `generation` is echoed back so the UI can ignore events
    /// from a scan it has already abandoned (e.g. after Rescan).
    Scan {
        generation: u64,
    },
    Execute {
        plan: Plan,
        dry_run: bool,
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
    HistoryLoaded {
        runs: Vec<StoredRun>,
        skipped: usize,
        error: Option<String>,
    },
}

pub(crate) struct Worker {
    commands: Sender<Command>,
    events: Receiver<Event>,
}

impl Worker {
    pub(crate) fn send(&self, command: Command) {
        let _ = self.commands.send(command);
    }

    pub(crate) fn try_recv(&self) -> Option<Event> {
        self.events.try_recv().ok()
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
        Command::Scan { generation } => {
            if !*supported || !roots.has_required() {
                emit(Event::ScanDone {
                    generation,
                    outcome: Err(crate::strings::ENGLISH.unsupported_platform.to_owned()),
                });
                return;
            }
            tracing::info!(generation, "scan started");
            let plan = build_plan(registry, roots, |update| {
                emit(Event::ScanProgress { generation, update });
            });
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
        Command::Execute { plan, dry_run } => {
            tracing::info!(selected = plan.selected, dry_run, "cleanup started");
            let result = execute_with_result(
                &plan,
                Options { dry_run },
                &roots.guard_roots(),
                recycler,
                |update| emit(Event::DeleteProgress(update)),
            );
            let mut result = result;
            let stats_error = if dry_run {
                None
            } else {
                write_stats(stats_dir.as_deref(), &mut result)
            };
            tracing::info!(
                total_selected = result.total_selected,
                total_bytes = result.total_bytes,
                error_count = result.error_count,
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

/// Loads the stats directory into a [`Event::HistoryLoaded`], most recent
/// cleanup first.
fn load_history(stats_dir: Option<&std::path::Path>) -> Event {
    let Some(dir) = stats_dir else {
        return Event::HistoryLoaded {
            runs: Vec::new(),
            skipped: 0,
            error: Some("stats directory unavailable".to_owned()),
        };
    };
    match cleaner_core::load_stats(dir) {
        Ok((mut runs, skipped)) => {
            runs.sort_by_key(|run| std::cmp::Reverse(run.result.run_timestamp()));
            Event::HistoryLoaded {
                runs,
                skipped,
                error: None,
            }
        }
        Err(err) => Event::HistoryLoaded {
            runs: Vec::new(),
            skipped: 0,
            error: Some(err.to_string()),
        },
    }
}

fn write_stats(stats_dir: Option<&std::path::Path>, result: &mut ExecResult) -> Option<String> {
    let Some(dir) = stats_dir else {
        return Some("stats directory unavailable".to_owned());
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
        local_app_data: Some(root.join("AppData/Local")),
        roaming_app_data: Some(root.join("AppData/Roaming")),
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

        process_command(&mut runtime, Command::Scan { generation: 9 }, |event| {
            events.push(event);
        });

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
        process_command(&mut runtime, Command::Scan { generation: 1 }, |event| {
            events.push(event);
        });
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
        process_command(&mut runtime, Command::Scan { generation: 2 }, |event| {
            events.push(event);
        });
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
                app: "App".into(),
                label: "cache".into(),
                paths: vec![cache.clone()],
                errs: Vec::new(),
                bytes: 3,
                on: true,
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
        let Event::HistoryLoaded { runs, skipped, .. } = events.pop().unwrap() else {
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
            Event::HistoryLoaded { error: Some(_), .. }
        ));
        let (tx, rx) = crossbeam_channel::unbounded();
        drop(tx);
        run_commands(&rx, &mut runtime, |_| panic!("no events expected"));
    }
}
