//! The selection screen's main pane (search + sort toolbar and the per-category
//! app cards), its status bar, and the preview/confirm/details modals. Sidebar
//! navigation lives in `sidebar.rs`.

use cleaner_core::human_bytes;
use eframe::egui::{self, RichText, Ui};

use crate::app::{SelectModal, SelectState};
use crate::icons;
use crate::strings::UiText;
use crate::theme;
use crate::ui::components;
use crate::ui::components::CheckState;
use crate::viewmodel::{self, AppView, CategoryView, SortMode, ViewFilter, visible_categories};

pub(crate) enum SelectAction {
    Rescan,
    ConfirmedCleanup,
}

/// The central pane: search/sort toolbar, the scrollable category sections, and
/// the modals.
pub(crate) fn show(ui: &mut Ui, texts: &UiText, state: &mut SelectState) -> Option<SelectAction> {
    let mut action = None;

    toolbar(ui, texts, state);
    ui.add_space(theme::SPACE_SM);
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            body(ui, texts, state);
        });

    modals(ui.ctx(), texts, state, &mut action);
    action
}

/// The bottom status bar: plan overview plus the Show empty / Preview toggles
/// and Rescan.
pub(crate) fn statusbar(
    ui: &mut Ui,
    texts: &UiText,
    state: &mut SelectState,
) -> Option<SelectAction> {
    let mut action = None;
    let (apps, items, bytes) = viewmodel::plan_overview(&state.plan);
    let empty = viewmodel::empty_target_count(&state.plan);

    ui.horizontal(|ui| {
        ui.label(RichText::new(texts.cache_overview(apps, items, bytes)).color(theme::MUTED));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui
                .button(icons::with_label(icons::RESCAN, texts.action_rescan))
                .clicked()
            {
                action = Some(SelectAction::Rescan);
            }
            if ui
                .selectable_label(
                    state.dry_run,
                    icons::with_label(icons::PREVIEW, texts.toggle_preview_only),
                )
                .clicked()
            {
                state.dry_run = !state.dry_run;
            }
            if ui
                .selectable_label(state.show_empty, texts.show_empty_label(empty))
                .clicked()
            {
                state.show_empty = !state.show_empty;
            }
        });
    });
    action
}

fn toolbar(ui: &mut Ui, texts: &UiText, state: &mut SelectState) {
    ui.horizontal(|ui| {
        // Master checkbox: selects or clears every item currently shown, which
        // with hidden empty targets equals the old "Select non-empty".
        let categories = visible(texts, state);
        let (selected, total) = categories
            .iter()
            .fold((0, 0), |(selected, total), category| {
                let (s, t) = category_selection_counts(state, category);
                (selected + s, total + t)
            });
        if components::tri_checkbox(ui, components::check_state(selected, total))
            .on_hover_text(texts.tooltip_select_visible)
            .clicked()
        {
            let select = selected < total;
            for category in &categories {
                for app in &category.apps {
                    for &index in &app.indices {
                        state.plan.groups[index].on = select;
                    }
                }
            }
        }
        ui.label(
            RichText::new(icons::SEARCH)
                .size(theme::ICON_MD)
                .color(theme::MUTED),
        );
        ui.add(
            egui::TextEdit::singleline(&mut state.filter)
                .hint_text(texts.cache_search_hint)
                .desired_width(260.0),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            egui::ComboBox::from_id_salt("cache-sort")
                .selected_text(icons::with_label(
                    icons::SORT,
                    sort_label(texts, state.sort),
                ))
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut state.sort, SortMode::Name, texts.cache_sort_name);
                    ui.selectable_value(
                        &mut state.sort,
                        SortMode::SizeDesc,
                        texts.cache_sort_largest,
                    );
                    ui.selectable_value(
                        &mut state.sort,
                        SortMode::SizeAsc,
                        texts.cache_sort_smallest,
                    );
                });
        });
    });
}

fn sort_label(texts: &UiText, sort: SortMode) -> &str {
    match sort {
        SortMode::Name => texts.cache_sort_name,
        SortMode::SizeDesc => texts.cache_sort_largest,
        SortMode::SizeAsc => texts.cache_sort_smallest,
    }
}

/// The categories currently shown in the main pane.
fn visible(texts: &UiText, state: &SelectState) -> Vec<CategoryView> {
    visible_categories(
        texts,
        &state.plan,
        &ViewFilter {
            category: state.selected_category,
            search: &state.filter,
            sort: state.sort,
            show_empty: state.show_empty,
        },
    )
}

fn body(ui: &mut Ui, texts: &UiText, state: &mut SelectState) {
    let categories = visible(texts, state);
    if categories.is_empty() {
        components::centered_status(ui, texts.no_matching_cache_targets);
        return;
    }
    for category in &categories {
        category_section(ui, texts, state, category);
    }
}

fn category_section(ui: &mut Ui, texts: &UiText, state: &mut SelectState, category: &CategoryView) {
    let (selected, total) = category_selection_counts(state, category);

    let mut toggle = false;
    ui.horizontal(|ui| {
        toggle = components::tri_checkbox(ui, components::check_state(selected, total)).clicked();
        ui.label(
            RichText::new(icons::category_glyph(category.category))
                .size(theme::ICON_LG)
                .color(theme::MUTED),
        );
        ui.label(
            RichText::new(&category.name)
                .family(theme::bold())
                .size(theme::FONT_HEADING),
        );
        ui.label(RichText::new(texts.apps_count(category.apps.len())).color(theme::MUTED));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(components::size_text(texts, category.bytes));
        });
    });
    if toggle {
        let select = selected < total || total == 0;
        for app in &category.apps {
            for &index in &app.indices {
                state.plan.groups[index].on = select;
            }
        }
    }

    ui.add_space(theme::SPACE_XS);
    for app in &category.apps {
        app_card(ui, texts, state, app);
    }
    ui.add_space(theme::SPACE_MD);
}

fn app_card(ui: &mut Ui, texts: &UiText, state: &mut SelectState, app: &AppView) {
    let selected = app
        .indices
        .iter()
        .filter(|&&index| state.plan.groups[index].on)
        .count();
    let total = app.indices.len();

    let mut toggle = false;
    components::surface_frame().show(ui, |ui| {
        ui.set_min_width(ui.available_width());
        ui.horizontal(|ui| {
            toggle =
                components::tri_checkbox(ui, components::check_state(selected, total)).clicked();
            ui.label(RichText::new(&app.app).family(theme::bold()));
            ui.label(RichText::new(texts.selected_of_count(selected, total)).color(theme::MUTED));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(components::size_text(texts, app.bytes));
            });
        });
        for (row, &index) in app.indices.iter().enumerate() {
            item_row(ui, texts, state, index, row % 2 == 1);
        }
    });
    if toggle {
        let select = selected < total;
        for &index in &app.indices {
            state.plan.groups[index].on = select;
        }
    }
    ui.add_space(theme::SPACE_SM);
}

fn item_row(ui: &mut Ui, texts: &UiText, state: &mut SelectState, index: usize, striped: bool) {
    let mut open_details = false;
    components::striped_row(ui, striped, |ui| {
        let group = &state.plan.groups[index];
        let on = group.on;
        let bytes = group.bytes;
        let empty = viewmodel::is_empty_target(group);
        let has_errs = !group.errs.is_empty();
        let err_count = group.errs.len();
        let mut label = RichText::new(&group.label);
        if empty {
            label = label.color(theme::MUTED);
        }

        let changed = components::tri_checkbox(
            ui,
            if on {
                CheckState::Checked
            } else {
                CheckState::Unchecked
            },
        )
        .clicked();
        ui.label(label);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(components::size_text(texts, bytes));
            if has_errs {
                let warn = egui::Button::new(
                    RichText::new(icons::WARNING)
                        .size(theme::ICON_MD)
                        .color(theme::DANGER),
                )
                .frame(false);
                if ui
                    .add(warn)
                    .on_hover_text(texts.details_with_issues(err_count))
                    .clicked()
                {
                    open_details = true;
                }
            } else if components::icon_button(ui, icons::DETAILS, texts.result_details).clicked() {
                open_details = true;
            }
        });
        if changed {
            state.plan.groups[index].on = !on;
        }
    });
    if open_details {
        state.modal = Some(SelectModal::GroupDetails(index));
    }
}

fn category_selection_counts(state: &SelectState, category: &CategoryView) -> (usize, usize) {
    let mut selected = 0;
    let mut total = 0;
    for app in &category.apps {
        for &index in &app.indices {
            total += 1;
            if state.plan.groups[index].on {
                selected += 1;
            }
        }
    }
    (selected, total)
}

fn modals(
    ctx: &egui::Context,
    texts: &UiText,
    state: &mut SelectState,
    action: &mut Option<SelectAction>,
) {
    match state.modal {
        None => {}
        Some(SelectModal::NothingSelected) => {
            if components::text_modal(
                ctx,
                "nothing-selected",
                texts.dialog_nothing_selected_title,
                texts.dialog_select_cache_group,
                texts.dialog_close,
            ) {
                state.modal = None;
            }
        }
        Some(SelectModal::GroupDetails(index)) => {
            let Some(group) = state.plan.groups.get(index) else {
                state.modal = None;
                return;
            };
            let title = format!("{} - {}", group.app, group.label);
            let body = components::group_details_text(texts, group);
            if components::text_modal(ctx, "group-details", &title, &body, texts.dialog_close) {
                state.modal = None;
            }
        }
        Some(SelectModal::Preview) => {
            if preview_modal(ctx, texts, state) {
                state.modal = None;
            }
        }
        Some(SelectModal::Confirm) => {
            let mut close = false;
            let mut confirmed = false;
            let response = egui::Modal::new(egui::Id::new("confirm-cleanup")).show(ctx, |ui| {
                ui.set_max_width(480.0);
                ui.label(
                    RichText::new(texts.dialog_confirm_cache_title)
                        .family(theme::bold())
                        .size(theme::FONT_HEADING),
                );
                ui.separator();
                ui.label(texts.confirm_cache_cleanup(&state.plan));
                ui.separator();
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .add(components::accent_button(texts.action_clean_up))
                        .clicked()
                    {
                        confirmed = true;
                    }
                    if ui.button(texts.action_cancel).clicked() {
                        close = true;
                    }
                });
            });
            if confirmed {
                state.modal = None;
                *action = Some(SelectAction::ConfirmedCleanup);
            } else if close || response.should_close() {
                state.modal = None;
            }
        }
    }
}

/// The dry-run preview dialog: every selected group with its size, plus the
/// selection total. Returns `true` when it should close.
fn preview_modal(ctx: &egui::Context, texts: &UiText, state: &SelectState) -> bool {
    let mut close = false;
    let response = egui::Modal::new(egui::Id::new("dry-run-preview")).show(ctx, |ui| {
        ui.set_max_width(680.0);
        ui.label(
            RichText::new(texts.dialog_preview_title)
                .family(theme::bold())
                .size(theme::FONT_HEADING),
        );
        ui.separator();
        if state.plan.selected == 0 {
            ui.label(texts.dialog_preview_empty);
        } else {
            egui::ScrollArea::vertical()
                .max_height(420.0)
                .show(ui, |ui| {
                    ui.set_min_width(600.0);
                    for group in state.plan.groups.iter().filter(|group| group.on) {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(&group.app).family(theme::bold()));
                            ui.label(&group.label);
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    ui.label(components::size_text(texts, group.bytes));
                                },
                            );
                        });
                    }
                });
            ui.separator();
            ui.label(
                RichText::new(format!(
                    "{}  |  est. {}",
                    texts.items_count(state.plan.selected),
                    human_bytes(state.plan.total_bytes)
                ))
                .family(theme::bold()),
            );
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.button(texts.dialog_close).clicked() {
                close = true;
            }
        });
    });
    close || response.should_close()
}
