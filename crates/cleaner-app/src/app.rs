//! The eframe application: screen state machine, header, and event plumbing
//! between the UI and the background worker.

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use cleaner_core::{ExecResult, Plan, Rules, StoredRun};
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
use crate::worker::{self, Command, Event, Worker, WorkerStopped};

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
    /// How often to check for a new version, and what the user skipped.
    pub app_updates: app_update::Settings,
    /// Exclusions, age and size limits, and skipping running programs.
    pub rules: Rules,
}

impl Default for Prefs {
    fn default() -> Self {
        Self {
            remember_selection: true,
            show_empty: false,
            selection: Vec::new(),
            presets: Presets::default(),
            app_updates: app_update::Settings::default(),
            rules: Rules::default(),
        }
    }
}

const KEY_REMEMBER: &str = "remember_selection";
const KEY_SHOW_EMPTY: &str = "show_empty";
const KEY_SELECTION: &str = "selection";
const KEY_PRESETS: &str = "presets";
const KEY_APP_UPDATES: &str = "app_updates";
const KEY_RULES: &str = "rules";

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
        presets.migrate_keys();
        let mut selection: Vec<TargetKey> =
            eframe::get_value(storage, KEY_SELECTION).unwrap_or(fallback.selection);
        crate::migrate::target_keys(&mut selection);
        let mut rules: Rules = eframe::get_value(storage, KEY_RULES).unwrap_or(fallback.rules);
        rules.sanitize();
        Self {
            remember_selection: eframe::get_value(storage, KEY_REMEMBER)
                .unwrap_or(fallback.remember_selection),
            show_empty: eframe::get_value(storage, KEY_SHOW_EMPTY).unwrap_or(fallback.show_empty),
            selection,
            presets,
            app_updates: eframe::get_value(storage, KEY_APP_UPDATES)
                .unwrap_or(fallback.app_updates),
            rules,
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
        eframe::set_value(storage, KEY_APP_UPDATES, &self.app_updates);
        eframe::set_value(storage, KEY_RULES, &self.rules);
    }
}

#[expect(
    clippy::struct_excessive_bools,
    reason = "one flag per open dialog, plus elevation"
)]
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
    update_open: bool,
    prefs: Prefs,
    updater: app_update::Updater,
    /// Shared with the running cleanup; the Stop button sets it.
    stop: Arc<AtomicBool>,
    /// Whether this copy runs with administrator rights.
    elevated: bool,
    /// The rules as the Settings dialog found them, to rescan on close when
    /// they changed. Cleared whenever the dialog is not open.
    settings_baseline: Option<Rules>,
    /// The rules as the file preview found them, likewise.
    preview_baseline: Option<Rules>,
    /// The file preview window, when open.
    files: Option<ui::files::FilesView>,
    /// Bumped for each file list request, so a late answer is ignored.
    file_request: u64,
    /// A message for the user, shown once: a title and a body.
    notice: Option<(String, String)>,
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
    /// Whether this copy runs with administrator rights, which decides
    /// whether targets that need them carry a badge.
    pub elevated: bool,
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
            elevated: false,
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
    /// The user pressed Stop; the run ends after the path it is moving.
    pub stopping: bool,
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

pub(crate) enum HistoryState {
    /// Waiting for the worker to read the stats directory.
    Loading,
    /// The stats directory could not be read.
    Failed(String),
    Loaded(LoadedHistory),
}

pub(crate) struct LoadedHistory {
    pub runs: Vec<StoredRun>,
    pub skipped: usize,
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
    updates: bool,
    check_updates: bool,
    update_action: Option<app_update_ui::Action>,
    show_history: bool,
    refresh_history: bool,
    close_history: bool,
    delete_run: Option<PathBuf>,
    clear_history: bool,
    header_action: bool,
    rescan: bool,
    reset_selection: bool,
    execute: bool,
    stop: bool,
    list_files: Option<usize>,
    exclude: Option<PathBuf>,
    close_files: bool,
    elevate: bool,
    settings_action: Option<ui::settings::SettingsAction>,
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
            update_open: false,
            prefs: Prefs::load(cc.storage),
            updater: app_update::Updater::new(crate::APP_INFO, {
                let ctx = cc.egui_ctx.clone();
                move || ctx.request_repaint()
            }),
            stop: Arc::new(AtomicBool::new(false)),
            elevated: cleaner_platform::elevation::is_elevated(),
            settings_baseline: None,
            preview_baseline: None,
            files: None,
            file_request: 0,
            notice: None,
        };
        // UAC can elevate as an administrator account other than the one
        // signed in. That copy would find the other account's folders, so it
        // cleans nothing at all.
        if let Some((process, session)) = cleaner_platform::elevation::elevated_as_someone_else() {
            tracing::warn!(%process, %session, "running as another account");
            app.screen = Screen::Unsupported(app.texts.other_account(&process, &session));
            return app;
        }
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
            rules: self.prefs.rules.clone(),
        });
    }

    fn show_history(&mut self) {
        self.history = Some(HistoryState::Loading);
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
                    stopping: false,
                });
                self.stop = Arc::new(AtomicBool::new(false));
                self.worker.send(Command::Execute {
                    plan: state.plan,
                    dry_run: false,
                    rules: self.prefs.rules.clone(),
                    stop: Arc::clone(&self.stop),
                });
            }
            other => self.screen = other,
        }
    }

    fn drain_events(&mut self) {
        loop {
            match self.worker.try_recv() {
                Ok(Some(event)) => self.apply_event(event),
                Ok(None) => return,
                Err(WorkerStopped) => {
                    // Without this, a scan or cleanup in progress would wait
                    // forever and keep the update restart locked.
                    if !matches!(self.screen, Screen::Unsupported(_)) {
                        tracing::error!("the background worker stopped");
                        self.screen = Screen::Unsupported(self.texts.worker_stopped.to_owned());
                    }
                    self.history = None;
                    return;
                }
            }
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
            Event::FilesListed {
                request,
                files,
                truncated,
            } => {
                if let Some(view) = &mut self.files
                    && request == self.file_request
                {
                    view.loaded(files, truncated);
                }
            }
            Event::HistoryLoaded { outcome } => {
                if let Some(history) = &mut self.history {
                    *history = match outcome {
                        Ok((runs, skipped)) => HistoryState::Loaded(LoadedHistory {
                            runs,
                            skipped,
                            selected: 0,
                            confirm_clear: false,
                        }),
                        Err(message) => HistoryState::Failed(message),
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
    /// Opens the file preview for one target and asks the worker for its
    /// files under the current rules.
    fn list_files(&mut self, index: usize) {
        let Screen::Select(state) = &self.screen else {
            return;
        };
        let Some(group) = state.plan.groups.get(index) else {
            return;
        };
        self.file_request += 1;
        self.preview_baseline = Some(self.prefs.rules.clone());
        self.files = Some(ui::files::FilesView::new(format!(
            "{} - {}",
            group.app, group.label
        )));
        self.worker.send(Command::ListFiles {
            request: self.file_request,
            targets: group.targets.clone(),
            rules: self.prefs.rules.clone(),
        });
    }

    /// Adds `path` to the exclusions, and drops it from the open preview.
    fn exclude(&mut self, path: &std::path::Path) {
        if !self.prefs.rules.is_excluded(path) {
            self.prefs.rules.exclusions.push(path.to_path_buf());
            self.prefs.rules.sanitize();
        }
        if let Some(view) = &mut self.files {
            view.exclude(path);
        }
    }

    fn settings_action(&mut self, action: ui::settings::SettingsAction) {
        use ui::settings::SettingsAction;
        match action {
            SettingsAction::ResetSelection => {}
            SettingsAction::AddFolder => {
                if let Some(path) = crate::pickers::pick_folder(self.texts.exclusion_pick_folder) {
                    self.exclude(&path);
                }
            }
            SettingsAction::AddFile => {
                if let Some(path) = crate::pickers::pick_file(self.texts.exclusion_pick_file) {
                    self.exclude(&path);
                }
            }
            SettingsAction::RemoveExclusion(index) => {
                if index < self.prefs.rules.exclusions.len() {
                    self.prefs.rules.exclusions.remove(index);
                }
            }
            SettingsAction::Export => self.export_settings(),
            SettingsAction::Import => self.import_settings(),
        }
    }

    fn export_settings(&mut self) {
        if let Screen::Select(state) = &self.screen {
            self.prefs.remember(state);
        }
        let Some(path) = crate::pickers::save_settings_file(self.texts.settings_export) else {
            return;
        };
        let file = crate::settings_file::SettingsFile::new(
            &self.prefs.rules,
            &self.prefs.presets,
            self.prefs.remember_selection,
            self.prefs.show_empty,
            &self.prefs.selection,
        );
        let notice = match std::fs::write(&path, file.to_json()) {
            Ok(()) => self.texts.settings_exported(&path),
            Err(error) => self.texts.settings_export_failed(&error.to_string()),
        };
        self.notice = Some((self.texts.settings_export.to_owned(), notice));
    }

    /// Reads a settings file and, once all of it checks out, puts it in
    /// place and scans again. Nothing is cleaned.
    fn import_settings(&mut self) {
        let Some(path) = crate::pickers::open_settings_file(self.texts.settings_import) else {
            return;
        };
        let read = std::fs::File::open(&path).and_then(|file| {
            use std::io::Read as _;
            let mut bytes = Vec::new();
            file.take(crate::settings_file::MAX_BYTES + 1)
                .read_to_end(&mut bytes)
                .map(|_| bytes)
        });
        let parsed = read
            .map_err(|error| self.texts.settings_import_failed(&error.to_string()))
            .and_then(|bytes| {
                crate::settings_file::SettingsFile::parse(&bytes)
                    .map_err(|error| self.texts.settings_import_refused(&error))
            });
        match parsed {
            Ok(file) => {
                self.prefs.rules = file.rules;
                self.prefs.presets = file.presets;
                self.prefs.remember_selection = file.remember_selection;
                self.prefs.show_empty = file.show_empty;
                self.prefs.selection = file.selection;
                self.settings_baseline = None;
                self.settings_open = false;
                self.notice = Some((
                    self.texts.settings_import.to_owned(),
                    self.texts.settings_imported.to_owned(),
                ));
                self.history = None;
                self.start_scan();
            }
            Err(message) => {
                self.notice = Some((self.texts.settings_import.to_owned(), message));
            }
        }
    }

    /// Starts an elevated copy once this one's state is on disk, then
    /// closes. A declined prompt leaves this copy running.
    fn restart_elevated(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        if let Some(storage) = frame.storage_mut() {
            eframe::App::save(self, storage);
            storage.flush();
        }
        let executable = match std::env::current_exe() {
            Ok(path) => path,
            Err(error) => {
                self.notice = Some((
                    self.texts.elevate_title.to_owned(),
                    self.texts.elevate_failed(&error.to_string()),
                ));
                return;
            }
        };
        match cleaner_platform::elevation::start_elevated(&executable) {
            cleaner_platform::elevation::Elevation::Started => {
                tracing::info!("restarting with administrator rights");
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            cleaner_platform::elevation::Elevation::Declined => {
                self.notice = Some((
                    self.texts.elevate_title.to_owned(),
                    self.texts.elevate_declined.to_owned(),
                ));
            }
            cleaner_platform::elevation::Elevation::Failed(error) => {
                self.notice = Some((
                    self.texts.elevate_title.to_owned(),
                    self.texts.elevate_failed(&error),
                ));
            }
        }
    }

    fn restored_state(&self, plan: Plan) -> SelectState {
        let mut state = SelectState::new(plan);
        state.elevated = self.elevated;
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
            let (task, selection) = match history {
                HistoryState::Loading => (texts.task_history, String::new()),
                HistoryState::Failed(_) => (texts.task_history_failed, String::new()),
                HistoryState::Loaded(loaded) => {
                    (texts.task_history, texts.runs_count(loaded.runs.len()))
                }
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
            Screen::Results(state) => HeaderInfo {
                task: viewmodel::cleanup_result_headline(texts, &state.result).to_owned(),
                selection: texts.items_count(state.result.total_selected),
                savings: texts.freed_summary(state.result.freed()),
                savings_bytes: state.result.freed(),
                action: Some(HeaderAction {
                    label: texts.action_clean_again,
                    enabled: true,
                }),
            },
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
                    if ui::update::chip(ui, &self.updater, &self.prefs.app_updates) {
                        nav.updates = true;
                    }
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
                        Some(SelectAction::ListFiles(index)) => nav.list_files = Some(index),
                        Some(SelectAction::Elevate) => nav.elevate = true,
                        None => {}
                    },
                    Screen::Deleting(state) => {
                        if ui::progress::show(ui, texts, state) {
                            nav.stop = true;
                        }
                    }
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
            .show(root, |ui| match ui::select::statusbar(ui, texts, state) {
                Some(SelectAction::Rescan) => nav.rescan = true,
                Some(SelectAction::Elevate) => nav.elevate = true,
                _ => {}
            });
    }

    /// The Settings modal. It edits the live view directly, so toggling
    /// "List empty targets" updates the list behind it; `save` stores what the
    /// view ended up holding.
    fn draw_settings(&mut self, ctx: &egui::Context, nav: &mut Nav) {
        // However the dialog closed, its baseline goes with it, so the next
        // opening compares against the rules as they are then.
        if !self.settings_open {
            self.settings_baseline = None;
            return;
        }
        let Screen::Select(state) = &mut self.screen else {
            self.settings_open = false;
            self.settings_baseline = None;
            return;
        };
        if self.settings_baseline.is_none() {
            self.settings_baseline = Some(self.prefs.rules.clone());
        }
        let mut always_elevated = cleaner_platform::elevation::always_elevated();
        let was_always = always_elevated;
        let action = ui::settings::show(
            ctx,
            self.texts,
            ui::settings::Values {
                remember_selection: &mut self.prefs.remember_selection,
                show_empty: &mut state.show_empty,
                rules: &mut self.prefs.rules,
                always_elevated: &mut always_elevated,
            },
            &mut self.settings_open,
        );
        if always_elevated != was_always
            && let Err(error) = cleaner_platform::elevation::set_always_elevated(always_elevated)
        {
            self.notice = Some((self.texts.settings_title.to_owned(), error.to_string()));
        }
        match action {
            Some(ui::settings::SettingsAction::ResetSelection) => nav.reset_selection = true,
            Some(other) => nav.settings_action = Some(other),
            None => {}
        }
        // Closing the dialog after changing a rule scans again, so the sizes
        // and selection reflect the new rules.
        if !self.settings_open
            && let Some(before) = self.settings_baseline.take()
            && before != self.prefs.rules
        {
            nav.rescan = true;
        }
    }

    /// The app updates modal. The install is gated on this: a restart in the
    /// middle of a scan or a cleanup would cut it short.
    fn draw_updates(&mut self, ctx: &egui::Context, nav: &mut Nav) {
        let busy = matches!(self.screen, Screen::Scanning { .. } | Screen::Deleting(_));
        nav.update_action = ui::update::show(
            ctx,
            self.texts,
            &self.updater,
            &mut self.prefs.app_updates,
            busy,
            &mut self.update_open,
        );
    }

    fn apply_update_action(&mut self, ctx: &egui::Context, action: app_update_ui::Action) {
        if app_update_ui::apply(action, ctx, &mut self.updater, &mut self.prefs.app_updates) {
            self.update_open = false;
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

    fn handle_nav(&mut self, ctx: &egui::Context, mut nav: Nav) {
        if nav.stop {
            self.stop.store(true, Ordering::Relaxed);
            if let Screen::Deleting(state) = &mut self.screen {
                state.stopping = true;
            }
        }
        if let Some(index) = nav.list_files {
            self.list_files(index);
        }
        if let Some(path) = nav.exclude {
            self.exclude(&path);
        }
        if nav.close_files {
            self.files = None;
            // An exclusion made from the preview changes what the scan found.
            // The rescan below remembers the screen first, as Rescan does.
            if self
                .preview_baseline
                .take()
                .is_some_and(|before| before != self.prefs.rules)
            {
                nav.rescan = true;
            }
        }
        if let Some(action) = nav.settings_action {
            self.settings_action(action);
        }
        if nav.about {
            self.about_open = true;
        }
        if nav.settings {
            self.settings_open = true;
        }
        if nav.check_updates {
            // The About button promises a check, so start one as the modal
            // opens. The updater leaves a running check or download alone.
            self.updater.check_now();
            self.update_open = true;
        }
        if nav.updates {
            self.update_open = true;
        }
        if let Some(action) = nav.update_action {
            self.apply_update_action(ctx, action);
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
        // A scan replaces the selection screen, and `save` only reads that
        // screen. Take what it holds first, so preset edits and checkbox
        // changes made since the last save survive the rescan.
        if (nav.rescan || nav.reset_selection)
            && let Screen::Select(state) = &self.screen
        {
            self.prefs.remember(state);
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

    fn ui(&mut self, root: &mut egui::Ui, frame: &mut eframe::Frame) {
        self.drain_events();
        self.updater.poll(&mut self.prefs.app_updates);
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
        if ui::about::show(&ctx, self.texts, &mut self.about_open) {
            nav.check_updates = true;
        }
        self.draw_settings(&ctx, &mut nav);
        self.draw_updates(&ctx, &mut nav);
        if let Some(view) = &mut self.files {
            match ui::files::show(&ctx, self.texts, view) {
                Some(ui::files::FilesAction::Exclude(path)) => nav.exclude = Some(path),
                Some(ui::files::FilesAction::Close) => nav.close_files = true,
                None => {}
            }
        }
        if let Some((title, body)) = &self.notice
            && !ui::components::text_modal(&ctx, "notice", title, body, self.texts.dialog_close)
        {
            self.notice = None;
        }
        if nav.elevate {
            self.restart_elevated(&ctx, frame);
        }
        self.handle_nav(&ctx, nav);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cleaner_core::{Group, Options, ProgressUpdate};

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
                update_open: false,
                prefs: Prefs::default(),
                updater: app_update::Updater::new(crate::APP_INFO, || {}),
                stop: Arc::new(AtomicBool::new(false)),
                elevated: false,
                settings_baseline: None,
                preview_baseline: None,
                files: None,
                file_request: 0,
                notice: None,
            },
            commands,
            events,
        )
    }

    fn plan() -> Plan {
        let mut plan = Plan {
            groups: vec![Group {
                bytes: 42,
                on: true,
                ..Group::new("App", "cache")
            }],
            ..Plan::default()
        };
        plan.recompute_totals();
        plan
    }

    fn progress(current: usize, total: usize, message: &str) -> ProgressUpdate {
        ProgressUpdate {
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
            Command::Scan { generation: 1, .. }
        ));

        app.apply_event(Event::ScanProgress {
            generation: 0,
            update: progress(1, 1, "stale"),
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
            update: progress(1, 2, "App - cache"),
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
    fn a_rescan_keeps_the_selection_made_since_the_last_save() {
        let two = || {
            let mut plan = plan();
            let mut logs = plan.groups[0].clone();
            logs.label = "logs".into();
            plan.groups.push(logs);
            plan.recompute_totals();
            plan
        };
        let (mut app, commands, _) = app();
        app.start_scan();
        let _ = commands.recv().unwrap();
        app.apply_event(Event::ScanDone {
            generation: 1,
            outcome: Ok(two()),
        });
        let Screen::Select(state) = &mut app.screen else {
            panic!("expected the selection screen");
        };
        // The user clears one target, and no timed save runs before the rescan.
        state.plan.groups[0].on = false;
        app.handle_nav(
            &egui::Context::default(),
            Nav {
                rescan: true,
                ..Nav::default()
            },
        );
        app.apply_event(Event::ScanDone {
            generation: 2,
            outcome: Ok(two()),
        });
        let Screen::Select(state) = &app.screen else {
            panic!("expected the selection screen");
        };
        assert!(
            !state.plan.groups[0].on,
            "the rescan restored a stale selection"
        );
        assert!(state.plan.groups[1].on);
    }

    #[test]
    fn closing_the_preview_after_an_exclusion_keeps_selection_edits() {
        let two = || {
            let mut plan = plan();
            let mut logs = plan.groups[0].clone();
            logs.label = "logs".into();
            plan.groups.push(logs);
            plan.recompute_totals();
            plan
        };
        let (mut app, commands, _) = app();
        app.start_scan();
        let _ = commands.recv().unwrap();
        app.apply_event(Event::ScanDone {
            generation: 1,
            outcome: Ok(two()),
        });
        let Screen::Select(state) = &mut app.screen else {
            panic!("expected the selection screen");
        };
        state.plan.groups[0].on = false;
        app.list_files(1);
        let _ = commands.recv().unwrap();
        let ctx = egui::Context::default();
        app.handle_nav(
            &ctx,
            Nav {
                exclude: Some(PathBuf::from("/some/file")),
                ..Nav::default()
            },
        );
        app.handle_nav(
            &ctx,
            Nav {
                close_files: true,
                ..Nav::default()
            },
        );
        assert!(matches!(
            commands.recv().unwrap(),
            Command::Scan { generation: 2, .. }
        ));
        app.apply_event(Event::ScanDone {
            generation: 2,
            outcome: Ok(two()),
        });
        let Screen::Select(state) = &app.screen else {
            panic!("expected the selection screen");
        };
        assert!(
            !state.plan.groups[0].on,
            "the preview's rescan lost an edit"
        );
        assert!(state.plan.groups[1].on);
    }

    #[test]
    fn a_stopped_worker_ends_the_scan_in_progress() {
        let (mut app, _commands, events) = app();
        app.start_scan();
        app.history = Some(HistoryState::Loading);
        // Events already sent still arrive before the disconnect shows.
        events
            .send(Event::ScanProgress {
                generation: 1,
                update: progress(1, 2, "App - cache"),
            })
            .unwrap();
        drop(events);

        app.drain_events();
        assert!(
            matches!(&app.screen, Screen::Unsupported(message) if message == ENGLISH.worker_stopped)
        );
        assert!(
            app.history.is_none(),
            "nothing will answer the history load"
        );
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
            stopping: false,
            selected_bytes: 42,
        });
        app.apply_event(Event::DeleteProgress(progress(1, 1, "App - cache")));
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
            app_updates: app_update::Settings::default(),
            rules: Rules::default(),
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
            Command::Scan { generation: 1, .. }
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
            outcome: Ok((Vec::new(), 2)),
        });
        let Some(HistoryState::Loaded(history)) = &app.history else {
            panic!("expected loaded history");
        };
        assert_eq!(history.skipped, 2);
        assert_eq!(history.selected, 0);
    }
}
