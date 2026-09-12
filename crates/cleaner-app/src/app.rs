//! The eframe application: screen state machine, header, and event plumbing
//! between the UI and the background worker.

use std::collections::HashSet;
use std::path::PathBuf;

use cleaner_core::{ExecResult, Plan, StoredRun};
use eframe::egui::{self, RichText};

use crate::presets::{PresetError, Presets};
use crate::strings::{ENGLISH, UiText};
use crate::theme;
use crate::ui;
use crate::ui::history::HistoryAction;
use crate::ui::results::ResultsAction;
use crate::ui::select::SelectAction;
use crate::ui::sidebar::SidebarAction;
use crate::viewmodel::{self, Category, SortMode, TargetKey};
use crate::worker::{self, Command, Event, Worker};

/// What the app remembers between launches, held in eframe's storage.
///
/// Preview-only mode and empty-folder removal are deliberately absent. Both
/// stay per run, so a launch is never one click away from deleting.
pub(crate) struct Prefs {
    /// Whether the last selection is restored after a scan.
    pub remember_selection: bool,
    pub show_empty: bool,
    pub selection: Vec<TargetKey>,
    /// Named selections the user saved. One of them may be marked to apply at
    /// launch, which takes precedence over `selection`.
    pub presets: Presets,
}

impl Default for Prefs {
    fn default() -> Self {
        Self {
            remember_selection: true,
            show_empty: false,
            selection: Vec::new(),
            presets: Presets::default(),
        }
    }
}

const KEY_REMEMBER: &str = "remember_selection";
const KEY_SHOW_EMPTY: &str = "show_empty";
const KEY_SELECTION: &str = "selection";
const KEY_PRESETS: &str = "presets";

impl Prefs {
    fn load(storage: Option<&dyn eframe::Storage>) -> Self {
        let Some(storage) = storage else {
            return Self::default();
        };
        let fallback = Self::default();
        let mut presets: Presets =
            eframe::get_value(storage, KEY_PRESETS).unwrap_or(fallback.presets);
        // Storage is a file the user can edit, and an older build may have
        // written a shape this one no longer allows.
        presets.sanitize();
        Self {
            remember_selection: eframe::get_value(storage, KEY_REMEMBER)
                .unwrap_or(fallback.remember_selection),
            show_empty: eframe::get_value(storage, KEY_SHOW_EMPTY).unwrap_or(fallback.show_empty),
            selection: eframe::get_value(storage, KEY_SELECTION).unwrap_or(fallback.selection),
            presets,
        }
    }

    /// Copies what the selection screen holds, ready for the next `save`.
    /// Called on the autosave path and again before a cleanup takes the plan
    /// away.
    fn remember(&mut self, state: &SelectState) {
        self.show_empty = state.show_empty;
        // The selection screen owns the preset list while it is up, the same
        // way it owns `show_empty`.
        self.presets = state.presets.clone();
        let previous = std::mem::take(&mut self.selection);
        self.selection = if self.remember_selection {
            viewmodel::remembered_keys(&state.plan, &previous)
        } else {
            Vec::new()
        };
    }

    fn store(&self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, KEY_REMEMBER, &self.remember_selection);
        eframe::set_value(storage, KEY_SHOW_EMPTY, &self.show_empty);
        eframe::set_value(storage, KEY_SELECTION, &self.selection);
        eframe::set_value(storage, KEY_PRESETS, &self.presets);
    }
}

pub(crate) struct WinCleanerApp {
    texts: &'static UiText,
    worker: Worker,
    /// Bumped on every scan; events carrying an older generation are stale
    /// (e.g. from a scan abandoned via Rescan) and are dropped.
    generation: u64,
    screen: Screen,
    /// The history overlay. The underlying screen stays live so returning
    /// restores exactly where the user was.
    history: Option<HistoryState>,
    about_open: bool,
    settings_open: bool,
    prefs: Prefs,
}

pub(crate) enum Screen {
    Scanning {
        fraction: f32,
        status: String,
        task: String,
    },
    Select(SelectState),
    Deleting(DeletingState),
    Results(ResultsState),
    Unsupported(String),
}

pub(crate) struct SelectState {
    pub plan: Plan,
    /// The main-pane search text.
    pub filter: String,
    /// The sidebar-selected category, or `None` for "All".
    pub selected_category: Option<Category>,
    /// Category sections folded down to their header. All start expanded.
    pub collapsed: HashSet<Category>,
    pub sort: SortMode,
    /// Whether empty targets are revealed; hidden by default.
    pub show_empty: bool,
    /// Preview-only mode; enabled by default.
    pub dry_run: bool,
    /// The saved presets, edited in place here and copied back to [`Prefs`] on
    /// the next save.
    pub presets: Presets,
    pub modal: Option<SelectModal>,
}

impl SelectState {
    fn new(plan: Plan) -> Self {
        Self {
            plan,
            filter: String::new(),
            selected_category: None,
            collapsed: HashSet::new(),
            sort: SortMode::Name,
            show_empty: false,
            dry_run: true,
            presets: Presets::default(),
            modal: None,
        }
    }
}

pub(crate) enum SelectModal {
    NothingSelected,
    Preview,
    Confirm,
    GroupDetails(usize),
    /// Naming the current selection. Carries the field's text and the last
    /// refusal, so the dialog can say why nothing was saved.
    SavePreset {
        name: String,
        error: Option<PresetError>,
    },
    ManagePresets,
}

pub(crate) struct DeletingState {
    pub current: usize,
    pub total: usize,
    pub message: String,
    pub task: String,
    pub selected_bytes: u64,
}

pub(crate) struct ResultsState {
    pub result: ExecResult,
    pub stats_error: Option<String>,
    pub modal: Option<ResultsModal>,
}

pub(crate) enum ResultsModal {
    Errors,
    GroupDetails(usize),
}

#[derive(Default)]
pub(crate) struct HistoryState {
    pub loaded: bool,
    pub runs: Vec<StoredRun>,
    pub skipped: usize,
    pub error: Option<String>,
    pub selected: usize,
    /// Whether the clear-history confirmation dialog is open.
    pub confirm_clear: bool,
}

/// User intents collected while drawing a frame, applied afterwards so the
/// draw code never mutates navigation state mid-frame.
#[derive(Default)]
#[expect(clippy::struct_excessive_bools, reason = "one flag per UI intent")]
struct Nav {
    about: bool,
    settings: bool,
    show_history: bool,
    refresh_history: bool,
    close_history: bool,
    delete_run: Option<PathBuf>,
    clear_history: bool,
    header_action: bool,
    rescan: bool,
    reset_selection: bool,
    execute: bool,
}

struct HeaderInfo {
    task: String,
    selection: String,
    savings: String,
    savings_bytes: u64,
    action: Option<HeaderAction>,
}

struct HeaderAction {
    label: &'static str,
    enabled: bool,
}

impl WinCleanerApp {
    pub(crate) fn new(cc: &eframe::CreationContext<'_>) -> Self {
        egui_extras::install_image_loaders(&cc.egui_ctx);
        theme::apply(&cc.egui_ctx);
        let worker = worker::spawn(cc.egui_ctx.clone());
        let mut app = Self {
            texts: &ENGLISH,
            worker,
            generation: 0,
            screen: Screen::Unsupported(String::new()),
            history: None,
            about_open: false,
            settings_open: false,
            prefs: Prefs::load(cc.storage),
        };
        app.start_scan();
        app
    }

    fn start_scan(&mut self) {
        self.generation += 1;
        self.screen = Screen::Scanning {
            fraction: 0.0,
            status: self.texts.cache_scan_status.to_owned(),
            task: self.texts.task_cache_scanning.to_owned(),
        };
        self.worker.send(Command::Scan {
            generation: self.generation,
        });
    }

    fn show_history(&mut self) {
        self.history = Some(HistoryState::default());
        self.worker.send(Command::LoadHistory);
    }

    fn begin_execute(&mut self) {
        match std::mem::replace(&mut self.screen, Screen::Unsupported(String::new())) {
            Screen::Select(mut state) => {
                state.plan.recompute_totals();
                // The plan moves to the worker next, and `save` only reads the
                // selection screen. Without this, closing from the results
                // stores whatever was loaded at startup and loses the
                // selection the cleanup just ran with.
                self.prefs.remember(&state);
                self.screen = Screen::Deleting(DeletingState {
                    current: 0,
                    total: state.plan.selected,
                    message: self.texts.status_preparing.to_owned(),
                    task: self.texts.task_cache_deleting.to_owned(),
                    selected_bytes: state.plan.total_bytes,
                });
                self.worker.send(Command::Execute {
                    plan: state.plan,
                    dry_run: false,
                });
            }
            other => self.screen = other,
        }
    }

    fn drain_events(&mut self) {
        while let Some(event) = self.worker.try_recv() {
            self.apply_event(event);
        }
    }

    fn apply_event(&mut self, event: Event) {
        match event {
            Event::ScanProgress { generation, update } if generation == self.generation => {
                if let Screen::Scanning {
                    fraction,
                    status,
                    task,
                } = &mut self.screen
                {
                    if update.total > 0 {
                        *fraction = ui::components::fraction(update.current, update.total);
                    }
                    if !update.message.is_empty() {
                        *status = self.texts.cache_scan_progress(&update);
                        *task = self.texts.cache_scan_task_progress(&update);
                    }
                }
            }
            Event::ScanDone {
                generation,
                outcome,
            } if generation == self.generation => match outcome {
                Ok(plan) => self.screen = Screen::Select(self.restored_state(plan)),
                Err(message) => self.screen = Screen::Unsupported(message),
            },
            Event::DeleteProgress(update) => {
                if let Screen::Deleting(state) = &mut self.screen {
                    if update.total > 0 {
                        state.current = update.current;
                        state.total = update.total;
                        state.task = self.texts.cache_delete_task_progress(&update);
                    }
                    if !update.message.is_empty() {
                        state.message = update.message;
                    }
                }
            }
            Event::DeleteDone {
                result,
                stats_error,
            } => {
                self.screen = Screen::Results(ResultsState {
                    result,
                    stats_error,
                    modal: None,
                });
            }
            Event::HistoryLoaded {
                runs,
                skipped,
                error,
            } => {
                if let Some(history) = &mut self.history {
                    *history = HistoryState {
                        loaded: true,
                        runs,
                        skipped,
                        error,
                        selected: 0,
                        confirm_clear: false,
                    };
                }
            }
            // Stale scan events from an abandoned generation.
            Event::ScanProgress { .. } | Event::ScanDone { .. } => {}
        }
    }

    /// The selection screen for a fresh scan, with the remembered view and
    /// selection applied.
    ///
    /// A preset marked to apply at launch wins, because the user asked for
    /// that selection by name. Otherwise the last selection comes back.
    ///
    /// An empty stored selection means nothing is remembered, so the catalog
    /// defaults the scan computed stand. That makes a first launch and a reset
    /// behave alike, at the price of one case: someone who clears every
    /// checkbox and quits gets the defaults back rather than an empty list.
    fn restored_state(&self, plan: Plan) -> SelectState {
        let mut state = SelectState::new(plan);
        state.show_empty = self.prefs.show_empty;
        state.presets = self.prefs.presets.clone();
        if let Some(preset) = self.prefs.presets.on_start() {
            viewmodel::apply_saved_selection(&mut state.plan, &preset.keys);
        } else if self.prefs.remember_selection && !self.prefs.selection.is_empty() {
            viewmodel::apply_saved_selection(&mut state.plan, &self.prefs.selection);
        }
        state
    }

    fn header_info(&mut self) -> HeaderInfo {
        let texts = self.texts;
        let empty = |task: String, action: Option<HeaderAction>| HeaderInfo {
            task,
            selection: String::new(),
            savings: String::new(),
            savings_bytes: 0,
            action,
        };

        if let Some(history) = &self.history {
            let task = if history.error.is_some() {
                texts.task_history_failed
            } else {
                texts.task_history
            };
            let selection = if history.loaded && history.error.is_none() {
                texts.runs_count(history.runs.len())
            } else {
                String::new()
            };
            return HeaderInfo {
                task: task.to_owned(),
                selection,
                savings: String::new(),
                savings_bytes: 0,
                action: Some(HeaderAction {
                    label: texts.action_back,
                    enabled: true,
                }),
            };
        }

        match &mut self.screen {
            Screen::Scanning { task, .. } => empty(
                task.clone(),
                Some(HeaderAction {
                    label: texts.action_cancel,
                    enabled: true,
                }),
            ),
            Screen::Select(state) => {
                let (selection, savings) =
                    viewmodel::cache_selection_summary(texts, &mut state.plan);
                let label = if state.dry_run {
                    texts.action_preview
                } else {
                    texts.action_clean_up
                };
                HeaderInfo {
                    task: String::new(),
                    selection,
                    savings,
                    savings_bytes: state.plan.total_bytes,
                    action: Some(HeaderAction {
                        label,
                        enabled: true,
                    }),
                }
            }
            Screen::Deleting(state) => empty(state.task.clone(), None),
            Screen::Results(state) => {
                let (headline, _) = viewmodel::cleanup_result_summary(texts, &state.result, false);
                HeaderInfo {
                    task: headline,
                    selection: texts.items_count(state.result.total_selected),
                    savings: texts.freed_summary(state.result.total_bytes),
                    savings_bytes: state.result.total_bytes,
                    action: Some(HeaderAction {
                        label: texts.action_clean_again,
                        enabled: true,
                    }),
                }
            }
            Screen::Unsupported(_) => empty(String::new(), None),
        }
    }

    fn draw_header(&mut self, root: &mut egui::Ui, nav: &mut Nav) {
        let info = self.header_info();
        let texts = self.texts;
        egui::Panel::top("header")
            .frame(
                egui::Frame::new()
                    .fill(theme::SURFACE)
                    .inner_margin(egui::Margin::symmetric(12, 8)),
            )
            .show(root, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(texts.app_title)
                            .family(theme::bold())
                            .size(theme::FONT_HEADING),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if let Some(action) = &info.action {
                            // Fixed width so toggling the label (Preview ⇄
                            // Clean Up …) never shifts the header layout.
                            let button = ui::components::accent_button(action.label).min_size(
                                egui::vec2(theme::HEADER_ACTION_WIDTH, theme::CONTROL_HEIGHT),
                            );
                            if ui.add_enabled(action.enabled, button).clicked() {
                                nav.header_action = true;
                            }
                        }
                        if !info.savings.is_empty() {
                            ui::components::chip_frame().show(ui, |ui| {
                                ui.label(
                                    RichText::new(&info.savings)
                                        .family(theme::bold())
                                        .color(theme::magnitude_color(info.savings_bytes)),
                                );
                            });
                        }
                        if !info.selection.is_empty() {
                            ui::components::chip_frame().show(ui, |ui| {
                                ui.label(&info.selection);
                            });
                        }
                    });
                });
                if !info.task.is_empty() {
                    ui.label(RichText::new(&info.task).color(theme::MUTED));
                }
            });
    }

    fn draw_central(&mut self, root: &mut egui::Ui, nav: &mut Nav) {
        let texts = self.texts;
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(theme::BACKGROUND).inner_margin(12))
            .show(root, |ui| {
                if let Some(history) = &mut self.history {
                    match ui::history::show(ui, texts, history) {
                        Some(HistoryAction::Refresh) => nav.refresh_history = true,
                        Some(HistoryAction::DeleteRun(path)) => nav.delete_run = Some(path),
                        Some(HistoryAction::ClearAll) => nav.clear_history = true,
                        None => {}
                    }
                    return;
                }
                match &mut self.screen {
                    Screen::Scanning {
                        fraction, status, ..
                    } => ui::scan::show(ui, texts, *fraction, status),
                    Screen::Select(state) => match ui::select::show(ui, texts, state) {
                        Some(SelectAction::Rescan) => nav.rescan = true,
                        Some(SelectAction::ConfirmedCleanup) => nav.execute = true,
                        None => {}
                    },
                    Screen::Deleting(state) => ui::progress::show(ui, texts, state),
                    Screen::Results(state) => match ui::results::show(ui, texts, state) {
                        Some(ResultsAction::Done) => nav.rescan = true,
                        Some(ResultsAction::History) => nav.show_history = true,
                        None => {}
                    },
                    Screen::Unsupported(message) => ui::components::centered_status(ui, message),
                }
            });
    }

    /// The category sidebar. Only drawn on the selection screen, so callers
    /// must gate on `Screen::Select`. Stays up while history fills the main
    /// pane; picking a category then returns to the cleanup view.
    fn draw_sidebar(&mut self, root: &mut egui::Ui, nav: &mut Nav) {
        let texts = self.texts;
        let history_open = self.history.is_some();
        let Screen::Select(state) = &mut self.screen else {
            return;
        };
        egui::Panel::left("sidebar")
            .resizable(false)
            .exact_size(theme::SIDEBAR_WIDTH)
            .frame(
                egui::Frame::new()
                    .fill(theme::SURFACE)
                    .inner_margin(egui::Margin::symmetric(10, 12)),
            )
            .show(root, |ui| {
                match ui::sidebar::show(ui, texts, state, history_open) {
                    Some(SidebarAction::SelectCategory(category)) => {
                        state.selected_category = category;
                        nav.close_history = true;
                    }
                    Some(SidebarAction::History) => nav.show_history = true,
                    Some(SidebarAction::Settings) => nav.settings = true,
                    Some(SidebarAction::About) => nav.about = true,
                    None => {}
                }
            });
    }

    /// The bottom status bar with the plan overview and view toggles.
    fn draw_statusbar(&mut self, root: &mut egui::Ui, nav: &mut Nav) {
        let texts = self.texts;
        let Screen::Select(state) = &mut self.screen else {
            return;
        };
        egui::Panel::bottom("statusbar")
            .frame(
                egui::Frame::new()
                    .fill(theme::SURFACE)
                    .inner_margin(egui::Margin::symmetric(12, 8)),
            )
            .show(root, |ui| {
                if let Some(SelectAction::Rescan) = ui::select::statusbar(ui, texts, state) {
                    nav.rescan = true;
                }
            });
    }

    /// The Settings modal. It edits the live view directly, so toggling
    /// "List empty targets" updates the list behind it; `save` stores what the
    /// view ended up holding.
    fn draw_settings(&mut self, ctx: &egui::Context, nav: &mut Nav) {
        if !self.settings_open {
            return;
        }
        let Screen::Select(state) = &mut self.screen else {
            self.settings_open = false;
            return;
        };
        let action = ui::settings::show(
            ctx,
            self.texts,
            &mut self.prefs.remember_selection,
            &mut state.show_empty,
            &mut self.settings_open,
        );
        if let Some(ui::settings::SettingsAction::ResetSelection) = action {
            nav.reset_selection = true;
        }
    }

    fn on_header_action(&mut self, ctx: &egui::Context) {
        if self.history.is_some() {
            self.history = None;
            return;
        }
        match &mut self.screen {
            Screen::Scanning { .. } => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
            Screen::Select(state) => {
                state.plan.recompute_totals();
                state.modal = Some(if state.plan.selected == 0 {
                    SelectModal::NothingSelected
                } else if state.dry_run {
                    SelectModal::Preview
                } else {
                    SelectModal::Confirm
                });
            }
            Screen::Results(_) => self.start_scan(),
            Screen::Deleting(_) | Screen::Unsupported(_) => {}
        }
    }

    fn handle_nav(&mut self, ctx: &egui::Context, nav: Nav) {
        if nav.about {
            self.about_open = true;
        }
        if nav.settings {
            self.settings_open = true;
        }
        if nav.close_history {
            self.history = None;
        }
        if nav.show_history || nav.refresh_history {
            self.show_history();
        }
        // Delete/clear reload history themselves, so the view stays open and
        // refreshes when the worker finishes.
        if let Some(path) = nav.delete_run {
            self.worker.send(Command::DeleteRun(path));
        }
        if nav.clear_history {
            self.worker.send(Command::ClearHistory);
        }
        if nav.header_action {
            self.on_header_action(ctx);
        }
        // Clearing the stored selection is only half of it. `save` rewrites
        // that field from the live screen within 30 seconds, so the reset has
        // to reach the plan too, and a fresh scan is what puts the catalog
        // defaults back on it.
        if nav.reset_selection {
            self.prefs.selection.clear();
            self.settings_open = false;
        }
        if nav.rescan || nav.reset_selection {
            self.history = None;
            self.start_scan();
        }
        if nav.execute {
            self.begin_execute();
        }
    }
}

impl eframe::App for WinCleanerApp {
    /// Called on a timer and at exit. Reads the live selection screen, so what
    /// is stored is what the user last saw.
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        if let Screen::Select(state) = &self.screen {
            self.prefs.remember(state);
        }
        self.prefs.store(storage);
    }

    fn ui(&mut self, root: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.drain_events();
        let ctx = root.ctx().clone();
        let mut nav = Nav::default();
        self.draw_header(root, &mut nav);
        // Side and bottom panels must be registered before the central panel,
        // and only on the selection screen. The sidebar stays up while the
        // history view fills the main pane; the status bar is cleanup-specific.
        if matches!(self.screen, Screen::Select(_)) {
            self.draw_sidebar(root, &mut nav);
            if self.history.is_none() {
                self.draw_statusbar(root, &mut nav);
            }
        }
        self.draw_central(root, &mut nav);
        ui::about::show(&ctx, self.texts, &mut self.about_open);
        self.draw_settings(&ctx, &mut nav);
        self.handle_nav(&ctx, nav);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cleaner_core::{Group, Options, Phase, ProgressUpdate};

    fn app() -> (
        WinCleanerApp,
        crossbeam_channel::Receiver<Command>,
        crossbeam_channel::Sender<Event>,
    ) {
        let (worker, commands, events) = Worker::test_channels();
        (
            WinCleanerApp {
                texts: &ENGLISH,
                worker,
                generation: 0,
                screen: Screen::Unsupported(String::new()),
                history: None,
                about_open: false,
                settings_open: false,
                prefs: Prefs::default(),
            },
            commands,
            events,
        )
    }

    fn plan() -> Plan {
        let mut plan = Plan {
            groups: vec![Group {
                app: "App".into(),
                label: "cache".into(),
                paths: Vec::new(),
                errs: Vec::new(),
                bytes: 42,
                on: true,
            }],
            ..Plan::default()
        };
        plan.recompute_totals();
        plan
    }

    fn progress(phase: Phase, current: usize, total: usize, message: &str) -> ProgressUpdate {
        ProgressUpdate {
            phase,
            current,
            total,
            message: message.to_owned(),
        }
    }

    #[test]
    fn scan_state_tracks_current_generation_and_ignores_stale_events() {
        let (mut app, commands, _) = app();
        app.start_scan();
        assert_eq!(app.generation, 1);
        assert!(matches!(
            commands.recv().unwrap(),
            Command::Scan { generation: 1 }
        ));

        app.apply_event(Event::ScanProgress {
            generation: 0,
            update: progress(Phase::Scan, 1, 1, "stale"),
        });
        let Screen::Scanning {
            fraction, status, ..
        } = &app.screen
        else {
            panic!("expected scanning state");
        };
        assert!(fraction.abs() < f32::EPSILON);
        assert_ne!(status, "stale");

        app.apply_event(Event::ScanProgress {
            generation: 1,
            update: progress(Phase::Scan, 1, 2, "App - cache"),
        });
        let Screen::Scanning {
            fraction, status, ..
        } = &app.screen
        else {
            panic!("expected scanning state");
        };
        assert!((*fraction - 0.5).abs() < f32::EPSILON);
        assert!(status.contains("App - cache"));

        app.apply_event(Event::ScanDone {
            generation: 0,
            outcome: Err("stale failure".into()),
        });
        assert!(matches!(app.screen, Screen::Scanning { .. }));
        app.apply_event(Event::ScanDone {
            generation: 1,
            outcome: Ok(plan()),
        });
        assert!(matches!(app.screen, Screen::Select(_)));
    }

    #[test]
    fn scan_failure_delete_progress_and_completion_transition_screens() {
        let (mut app, _, _) = app();
        app.generation = 3;
        app.apply_event(Event::ScanDone {
            generation: 3,
            outcome: Err("unsupported".into()),
        });
        assert!(matches!(&app.screen, Screen::Unsupported(message) if message == "unsupported"));

        app.screen = Screen::Deleting(DeletingState {
            current: 0,
            total: 1,
            message: String::new(),
            task: String::new(),
            selected_bytes: 42,
        });
        app.apply_event(Event::DeleteProgress(progress(
            Phase::Delete,
            1,
            1,
            "App - cache",
        )));
        let Screen::Deleting(state) = &app.screen else {
            panic!("expected deleting state");
        };
        assert_eq!(state.current, 1);
        assert_eq!(state.message, "App - cache");

        let result = ExecResult::begin(&plan(), Options::default());
        app.apply_event(Event::DeleteDone {
            result,
            stats_error: Some("disk full".into()),
        });
        assert!(
            matches!(&app.screen, Screen::Results(state) if state.stats_error.as_deref() == Some("disk full"))
        );
    }

    #[test]
    fn a_scan_applies_the_remembered_view_and_selection() {
        let (mut remembered, _, _) = app();
        remembered.prefs = Prefs {
            remember_selection: true,
            show_empty: true,
            // A target that is no longer in the catalog is simply ignored.
            selection: vec![("Gone".to_owned(), "cache".to_owned())],
            presets: Presets::default(),
        };
        remembered.generation = 1;
        remembered.apply_event(Event::ScanDone {
            generation: 1,
            outcome: Ok(plan()),
        });
        let Screen::Select(state) = &remembered.screen else {
            panic!("expected the selection screen");
        };
        assert!(state.show_empty, "the remembered view comes back");
        assert!(
            !state.plan.groups[0].on,
            "a target left unselected stays unselected, whatever its default"
        );
        assert!(state.dry_run, "preview stays on at every launch");

        // Remembering off: the catalog defaults decide again.
        let (mut fresh, _, _) = app();
        fresh.prefs = Prefs {
            remember_selection: false,
            ..Prefs::default()
        };
        fresh.generation = 1;
        fresh.apply_event(Event::ScanDone {
            generation: 1,
            outcome: Ok(plan()),
        });
        let Screen::Select(state) = &fresh.screen else {
            panic!("expected the selection screen");
        };
        assert!(state.plan.groups[0].on);
    }

    #[test]
    fn a_first_launch_keeps_the_catalog_defaults() {
        // Remembering is on by default with nothing stored yet, which is also
        // the state a reset leaves behind.
        let (mut app, _, _) = app();
        assert!(app.prefs.remember_selection);
        assert!(app.prefs.selection.is_empty());
        app.generation = 1;
        app.apply_event(Event::ScanDone {
            generation: 1,
            outcome: Ok(plan()),
        });
        let Screen::Select(state) = &app.screen else {
            panic!("expected the selection screen");
        };
        assert!(
            state.plan.groups[0].on,
            "an empty stored selection leaves the scan's defaults alone"
        );
        assert_eq!(state.plan.selected, 1);
    }

    #[test]
    fn a_startup_preset_wins_over_the_last_selection() {
        let (mut app, _, _) = app();
        let mut presets = Presets::default();
        presets
            .save("Weekly", vec![("App".to_owned(), "cache".to_owned())])
            .unwrap();
        presets.set_on_start(0, true);
        app.prefs = Prefs {
            remember_selection: true,
            // The last selection says otherwise, and the preset overrules it.
            selection: vec![("Gone".to_owned(), "cache".to_owned())],
            presets,
            ..Prefs::default()
        };
        app.generation = 1;
        app.apply_event(Event::ScanDone {
            generation: 1,
            outcome: Ok(plan()),
        });
        let Screen::Select(state) = &app.screen else {
            panic!("expected the selection screen");
        };
        assert!(state.plan.groups[0].on, "the preset decided");
        assert_eq!(state.plan.selected, 1);
        assert_eq!(
            state.presets.as_slice().len(),
            1,
            "the screen carries the presets so the toolbar can edit them"
        );
        assert!(state.dry_run, "preview still stays on at every launch");
    }

    #[test]
    fn presets_edited_on_screen_reach_storage() {
        let (mut app, _, _) = app();
        let mut state = SelectState::new(plan());
        state.presets.save("Weekly", Vec::new()).unwrap();
        app.screen = Screen::Select(state);

        let Screen::Select(state) = &app.screen else {
            unreachable!()
        };
        app.prefs.remember(state);
        assert_eq!(app.prefs.presets.as_slice().len(), 1);
        assert_eq!(app.prefs.presets.get(0).unwrap().name, "Weekly");
    }

    #[test]
    fn resetting_the_selection_clears_it_and_rescans() {
        let (mut app, commands, _) = app();
        app.screen = Screen::Select(SelectState::new(plan()));
        app.settings_open = true;
        app.prefs.selection = vec![("App".to_owned(), "cache".to_owned())];

        app.handle_nav(
            &egui::Context::default(),
            Nav {
                reset_selection: true,
                ..Nav::default()
            },
        );
        assert!(app.prefs.selection.is_empty());
        assert!(!app.settings_open, "the modal has no screen to sit on");
        // Only a fresh scan can put the catalog defaults back on the plan.
        assert!(matches!(
            commands.recv().unwrap(),
            Command::Scan { generation: 1 }
        ));
        assert!(matches!(app.screen, Screen::Scanning { .. }));
    }

    #[test]
    fn execute_and_history_actions_emit_commands_and_update_state() {
        let (mut app, commands, _) = app();
        app.screen = Screen::Select(SelectState::new(plan()));
        app.begin_execute();
        assert!(matches!(app.screen, Screen::Deleting(_)));
        assert!(matches!(
            commands.recv().unwrap(),
            Command::Execute { dry_run: false, .. }
        ));
        // The plan is gone to the worker, so the selection has to be captured
        // before the screen leaves Select.
        assert_eq!(
            app.prefs.selection,
            vec![("App".to_owned(), "cache".to_owned())]
        );

        app.show_history();
        assert!(matches!(commands.recv().unwrap(), Command::LoadHistory));
        app.apply_event(Event::HistoryLoaded {
            runs: Vec::new(),
            skipped: 2,
            error: None,
        });
        let history = app.history.as_ref().unwrap();
        assert!(history.loaded);
        assert_eq!(history.skipped, 2);
        assert_eq!(history.selected, 0);
    }
}
