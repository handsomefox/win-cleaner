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
use crate::ui::presets;
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
        // Master checkbox: selects or clears every item the filters let
        // through, collapsed sections included. With empty targets hidden,
        // that equals the old "Select non-empty".
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
        if !state.filter.is_empty()
            && components::icon_button(ui, icons::CLEAR, texts.tooltip_clear_search).clicked()
        {
            state.filter.clear();
        }
        presets::menu(ui, texts, state);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            // Laid out right to left, so the last mode listed sits leftmost.
            for (mode, glyph, label) in [
                (
                    SortMode::SizeAsc,
                    icons::SORT_SMALLEST,
                    texts.cache_sort_smallest,
                ),
                (
                    SortMode::SizeDesc,
                    icons::SORT_LARGEST,
                    texts.cache_sort_largest,
                ),
                (SortMode::Name, icons::SORT_NAME, texts.cache_sort_name),
            ] {
                ui.selectable_value(&mut state.sort, mode, icons::with_label(glyph, label));
            }
            ui.separator();
            let all_collapsed = !categories.is_empty()
                && categories
                    .iter()
                    .all(|category| state.collapsed.contains(&category.category));
            let (glyph, label) = if all_collapsed {
                (icons::EXPAND_ALL, texts.action_expand_all)
            } else {
                (icons::COLLAPSE_ALL, texts.action_collapse_all)
            };
            if ui.button(icons::with_label(glyph, label)).clicked() {
                for category in &categories {
                    if all_collapsed {
                        state.collapsed.remove(&category.category);
                    } else {
                        state.collapsed.insert(category.category);
                    }
                }
            }
        });
    });
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

    let collapsed = state.collapsed.contains(&category.category);
    let mut toggle = false;
    let mut fold = false;
    ui.horizontal(|ui| {
        toggle = components::tri_checkbox(ui, components::check_state(selected, total)).clicked();
        let (caret, tooltip) = if collapsed {
            (icons::EXPAND, texts.tooltip_expand_section)
        } else {
            (icons::COLLAPSE, texts.tooltip_collapse_section)
        };
        fold = components::icon_button(ui, caret, tooltip).clicked();
        ui.label(
            RichText::new(icons::category_glyph(category.category))
                .size(theme::ICON_LG)
                .color(theme::category_color(category.category)),
        );
        // The name folds the section too, a bigger target than the caret.
        let name = egui::Label::new(
            RichText::new(&category.name)
                .family(theme::bold())
                .size(theme::FONT_HEADING),
        )
        .sense(egui::Sense::click());
        fold |= ui
            .add(name)
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .clicked();
        ui.label(RichText::new(texts.apps_count(category.apps.len())).color(theme::MUTED));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(components::size_text(category.bytes));
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
    if fold {
        if collapsed {
            state.collapsed.remove(&category.category);
        } else {
            state.collapsed.insert(category.category);
        }
    }
    if collapsed {
        ui.add_space(theme::SPACE_MD);
        return;
    }

    ui.add_space(theme::SPACE_XS);
    let columns = viewmodel::grid_columns(
        ui.available_width(),
        theme::CARD_MIN_WIDTH,
        ui.spacing().item_spacing.x,
        theme::GRID_MAX_COLUMNS,
    );
    // Each card goes to the shortest column so far, so a tall card leaves no
    // hole beside it. Cards of equal height still follow the sort order row
    // by row.
    let heights: Vec<usize> = category.apps.iter().map(AppView::card_rows).collect();
    let packed = viewmodel::pack_columns(&heights, columns);
    let tag = theme::category_color(category.category);
    ui.columns(columns, |cells| {
        for (cell, cards) in cells.iter_mut().zip(&packed) {
            for &card in cards {
                app_card(cell, texts, state, &category.apps[card], tag);
            }
        }
    });
    ui.add_space(theme::SPACE_MD);
}

fn app_card(
    ui: &mut Ui,
    texts: &UiText,
    state: &mut SelectState,
    app: &AppView,
    tag: egui::Color32,
) {
    let selected = app
        .indices
        .iter()
        .filter(|&&index| state.plan.groups[index].on)
        .count();
    let total = app.indices.len();

    let mut toggle = false;
    components::tagged_card(ui, tag, |ui| {
        ui.set_min_width(ui.available_width());
        // An app with one target needs no header: its one row names the app.
        if let [index] = app.indices[..] {
            item_row(ui, texts, state, index, Some(&app.app), false);
            return;
        }
        // Same padding as the target rows, so the checkboxes line up.
        components::striped_row(ui, false, |ui| {
            components::split_row(
                ui,
                |ui| {
                    toggle = components::tri_checkbox(ui, components::check_state(selected, total))
                        .clicked();
                    ui.add(
                        egui::Label::new(RichText::new(&app.app).family(theme::bold())).truncate(),
                    );
                    ui.label(
                        RichText::new(texts.selected_of_count(selected, total)).color(theme::MUTED),
                    );
                },
                |ui| {
                    ui.label(components::size_text(app.bytes));
                },
            );
        });
        for (row, &index) in app.indices.iter().enumerate() {
            item_row(ui, texts, state, index, None, row % 2 == 1);
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

/// One target row. `app` is set when the row stands in for a whole
/// single-target app card, and then leads the row in bold.
fn item_row(
    ui: &mut Ui,
    texts: &UiText,
    state: &mut SelectState,
    index: usize,
    app: Option<&str>,
    striped: bool,
) {
    let mut open_details = false;
    components::striped_row(ui, striped, |ui| {
        let group = &state.plan.groups[index];
        let on = group.on;
        let size = components::group_size_text(texts, group);
        let empty = viewmodel::is_empty_target(group);
        let has_errs = !group.errs.is_empty();
        let err_count = group.errs.len();
        let mut label = RichText::new(&group.label);
        if empty || app.is_some() {
            label = label.color(theme::MUTED);
        }

        let mut changed = false;
        components::split_row(
            ui,
            |ui| {
                changed = components::tri_checkbox(
                    ui,
                    if on {
                        CheckState::Checked
                    } else {
                        CheckState::Unchecked
                    },
                )
                .clicked();
                if let Some(app) = app {
                    ui.add(egui::Label::new(RichText::new(app).family(theme::bold())).truncate());
                }
                ui.add(egui::Label::new(label).truncate());
            },
            |ui| {
                ui.label(size);
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
                } else if components::icon_button(ui, icons::DETAILS, texts.result_details)
                    .clicked()
                {
                    open_details = true;
                }
            },
        );
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
    // Taken out for the frame so a dialog can hold its own editable state (the
    // preset name field) while it also borrows the rest of the screen.
    // Whatever is still open goes back at the end.
    let Some(modal) = state.modal.take() else {
        return;
    };
    match modal {
        SelectModal::NothingSelected => {
            if !components::text_modal(
                ctx,
                "nothing-selected",
                texts.dialog_nothing_selected_title,
                texts.dialog_select_cache_group,
                texts.dialog_close,
            ) {
                state.modal = Some(SelectModal::NothingSelected);
            }
        }
        SelectModal::GroupDetails(index) => {
            let Some(group) = state.plan.groups.get(index) else {
                return;
            };
            let title = format!("{} - {}", group.app, group.label);
            let body = components::group_details_text(texts, group);
            if !components::text_modal(ctx, "group-details", &title, &body, texts.dialog_close) {
                state.modal = Some(SelectModal::GroupDetails(index));
            }
        }
        SelectModal::Preview => {
            if !preview_modal(ctx, texts, state) {
                state.modal = Some(SelectModal::Preview);
            }
        }
        SelectModal::Confirm => {
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
                *action = Some(SelectAction::ConfirmedCleanup);
            } else if !(close || response.should_close()) {
                state.modal = Some(SelectModal::Confirm);
            }
        }
        SelectModal::SavePreset {
            mut name,
            mut error,
        } => {
            if !presets::save_dialog(ctx, texts, state, &mut name, &mut error) {
                state.modal = Some(SelectModal::SavePreset { name, error });
            }
        }
        SelectModal::ManagePresets => {
            if !presets::manage_dialog(ctx, texts, state) {
                state.modal = Some(SelectModal::ManagePresets);
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
                                    ui.label(components::group_size_text(texts, group));
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
