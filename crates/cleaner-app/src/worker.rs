//! The background worker: a single thread that owns the catalog, roots, and
//! recycler, and talks to the UI thread over channels. The UI never touches
//! the filesystem directly.

use std::path::PathBuf;

use cleaner_catalog::build_registry;
use cleaner_core::{
    ExecResult, Options, Plan, ProgressUpdate, Roots, StoredRun, build_plan, execute_with_result,
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
    let registry = build_registry(&roots);
    let recycler = ShellRecycler;
    let supported = cfg!(windows) || dev_root().is_some();

    let emit = |event: Event| {
        let _ = events.send(event);
        ctx.request_repaint();
    };

    while let Ok(command) = commands.recv() {
        match command {
            Command::Scan { generation } => {
                if !supported || !roots.has_required() {
                    emit(Event::ScanDone {
                        generation,
                        outcome: Err(crate::strings::ENGLISH.unsupported_platform.to_owned()),
                    });
                    continue;
                }
                tracing::info!(generation, "scan started");
                let plan = build_plan(&registry, &roots, |update| {
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
                    &recycler,
                    |update| emit(Event::DeleteProgress(update)),
                );
                let mut result = result;
                let stats_error = if dry_run {
                    None
                } else {
                    write_stats(&mut result)
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
            Command::LoadHistory => emit(load_history()),
            Command::DeleteRun(path) => {
                if let Err(err) = std::fs::remove_file(&path) {
                    tracing::warn!("failed to delete run {}: {err}", path.display());
                }
                emit(load_history());
            }
            Command::ClearHistory => {
                if let Some(dir) = cleaner_platform::stats_dir()
                    && let Err(err) = cleaner_core::clear_stats(&dir)
                {
                    tracing::warn!("failed to clear history: {err}");
                }
                emit(load_history());
            }
        }
    }
}

/// Loads the stats directory into a [`Event::HistoryLoaded`], most recent
/// cleanup first.
fn load_history() -> Event {
    let Some(dir) = cleaner_platform::stats_dir() else {
        return Event::HistoryLoaded {
            runs: Vec::new(),
            skipped: 0,
            error: Some("stats directory unavailable".to_owned()),
        };
    };
    match cleaner_core::load_stats(&dir) {
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

fn write_stats(result: &mut ExecResult) -> Option<String> {
    let Some(dir) = cleaner_platform::stats_dir() else {
        return Some("stats directory unavailable".to_owned());
    };
    match cleaner_core::write_stats(&dir, result) {
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
