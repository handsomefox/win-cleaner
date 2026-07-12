//! The selection screen: toolbar plus the category → app → item tree, and the
//! preview/confirm/details modals.

use cleaner_core::human_bytes;
use eframe::egui::{self, RichText, Ui};

use crate::app::{SelectModal, SelectState};
use crate::icons::category_icon;
use crate::strings::UiText;
use crate::theme;
use crate::ui::components;
use crate::viewmodel::{AppView, CategoryView, SortMode, categorized_app_groups};

pub(crate) enum SelectAction {
    Rescan,
    ConfirmedCleanup,
}

pub(crate) fn show(ui: &mut Ui, texts: &UiText, state: &mut SelectState) -> Option<SelectAction> {
    let mut action = None;

    components::surface_frame().show(ui, |ui| {
        ui.set_min_width(ui.available_width());
        toolbar(ui, texts, state, &mut action);
        ui.separator();
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                tree(ui, texts, state);
            });
    });

    modals(ui.ctx(), texts, state, &mut action);
    action
}

fn toolbar(
    ui: &mut Ui,
    texts: &UiText,
    state: &mut SelectState,
    action: &mut Option<SelectAction>,
) {
    ui.horizontal(|ui| {
        if ui.button(texts.action_select_all).clicked() {
            for group in &mut state.plan.groups {
                group.on = true;
            }
        }
        if ui.button(texts.action_select_non_empty).clicked() {
            for group in &mut state.plan.groups {
                group.on = group.bytes > 0;
            }
        }
        if ui.button(texts.action_deselect_all).clicked() {
            for group in &mut state.plan.groups {
                group.on = false;
            }
        }
        ui.add(
            egui::TextEdit::singleline(&mut state.filter)
                .hint_text(texts.cache_search_hint)
                .desired_width(220.0),
        );

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.button(texts.action_rescan).clicked() {
                *action = Some(SelectAction::Rescan);
            }
            let preview = ui.selectable_label(state.dry_run, texts.toggle_preview_only);
            if preview.clicked() {
                state.dry_run = !state.dry_run;
            }
            ui.separator();
            if ui.button(texts.action_collapse_all).clicked() {
                set_all_expanded(texts, state, false);
            }
            if ui.button(texts.action_expand_all).clicked() {
                set_all_expanded(texts, state, true);
            }
            egui::ComboBox::from_id_salt("cache-sort")
                .selected_text(sort_label(texts, state.sort))
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

fn set_all_expanded(texts: &UiText, state: &mut SelectState, expanded: bool) {
    for group in state.plan.by_app() {
        state.expanded_apps.insert(group.app, expanded);
    }
    for category in categorized_app_groups(texts, &state.plan, &state.filter, state.sort) {
        state.expanded_categories.insert(category.name, expanded);
    }
}

fn tree(ui: &mut Ui, texts: &UiText, state: &mut SelectState) {
    let categories = categorized_app_groups(texts, &state.plan, &state.filter, state.sort);
    if categories.is_empty() {
        components::centered_status(ui, texts.no_matching_cache_targets);
        return;
    }
    for (index, category) in categories.iter().enumerate() {
        category_section(ui, texts, state, category, index % 2 == 1);
    }
}

fn category_section(
    ui: &mut Ui,
    texts: &UiText,
    state: &mut SelectState,
    category: &CategoryView,
    striped: bool,
) {
    let (selected, total) = category_selection_counts(state, category);
    let expanded = *state
        .expanded_categories
        .get(&category.name)
        .unwrap_or(&false);

    let mut toggle_selection = false;
    let mut toggle_expanded = false;
    components::tree_row(ui, 0, striped, |ui| {
        toggle_selection =
            components::tri_checkbox(ui, components::check_state(selected, total)).clicked();
        let chevron =
            components::flat_button(ui, RichText::new(components::expand_chevron(expanded)));
        ui.label(
            RichText::new(category_icon(texts, &category.name))
                .size(theme::ICON_MD)
                .color(theme::MUTED),
        );
        let name = components::flat_button(
            ui,
            RichText::new(&category.name)
                .family(theme::bold())
                .size(theme::FONT_BODY),
        );
        toggle_expanded = chevron.clicked() || name.clicked();
        ui.label(RichText::new(texts.apps_count(category.apps.len())).color(theme::MUTED));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(components::size_text(texts, category.bytes));
        });
    });

    if toggle_selection {
        let select = selected < total || total == 0;
        for app in &category.apps {
            for &index in &app.indices {
                state.plan.groups[index].on = select;
            }
        }
    }
    if toggle_expanded {
        state
            .expanded_categories
            .insert(category.name.clone(), !expanded);
    }

    if *state
        .expanded_categories
        .get(&category.name)
        .unwrap_or(&false)
    {
        for (index, app) in category.apps.iter().enumerate() {
            app_section(ui, texts, state, app, index % 2 == 1);
        }
    }
}

fn app_section(ui: &mut Ui, texts: &UiText, state: &mut SelectState, app: &AppView, striped: bool) {
    let selected = app
        .indices
        .iter()
        .filter(|&&index| state.plan.groups[index].on)
        .count();
    let total = app.indices.len();
    let expanded = *state.expanded_apps.get(&app.app).unwrap_or(&false);

    let mut toggle_selection = false;
    let mut toggle_expanded = false;
    components::tree_row(ui, 1, striped, |ui| {
        toggle_selection =
            components::tri_checkbox(ui, components::check_state(selected, total)).clicked();
        let chevron =
            components::flat_button(ui, RichText::new(components::expand_chevron(expanded)));
        let name = components::flat_button(ui, RichText::new(&app.app).family(theme::bold()));
        toggle_expanded = chevron.clicked() || name.clicked();
        ui.label(RichText::new(texts.selected_of_count(selected, total)).color(theme::MUTED));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(components::size_text(texts, app.bytes));
        });
    });

    if toggle_selection {
        let select = selected < total;
        for &index in &app.indices {
            state.plan.groups[index].on = select;
        }
    }
    if toggle_expanded {
        state.expanded_apps.insert(app.app.clone(), !expanded);
    }

    if *state.expanded_apps.get(&app.app).unwrap_or(&false) {
        for (row, &index) in app.indices.iter().enumerate() {
            item_row(ui, texts, state, index, row % 2 == 1);
        }
    }
}

fn item_row(ui: &mut Ui, texts: &UiText, state: &mut SelectState, index: usize, striped: bool) {
    let mut open_details = false;
    components::tree_row(ui, 2, striped, |ui| {
        let group = &state.plan.groups[index];
        let on = group.on;
        let check = if on {
            components::CheckState::Checked
        } else {
            components::CheckState::Unchecked
        };
        let changed = components::tri_checkbox(ui, check).clicked();
        ui.label(&group.label);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(components::size_text(texts, group.bytes));
            let details = if group.errs.is_empty() {
                texts.result_details.to_owned()
            } else {
                texts.details_with_issues(group.errs.len())
            };
            open_details = ui.small_button(details).clicked();
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
