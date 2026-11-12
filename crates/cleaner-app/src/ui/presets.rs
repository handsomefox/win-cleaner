//! The Presets menu in the selection toolbar, plus the Save and Manage
//! dialogs behind it.
//!
//! Applying a preset goes through [`viewmodel::apply_saved_selection`], the
//! same path a remembered selection takes, so the empty-target rule holds for
//! a preset too. Picking a preset by hand also checks the empty-folder rows
//! it holds: that pick is the user's own click. The startup preset leaves
//! them unchecked.

use eframe::egui::{self, RichText, Ui};

use crate::app::{SelectModal, SelectState};
use crate::icons;
use crate::presets::{MAX_PRESETS, PresetError};
use crate::strings::UiText;
use crate::theme;
use crate::viewmodel::{self, EmptyFolderRows, PresetTargetStatus, TargetKey};

/// How many of a preset's targets its menu entry lists on hover. Manage
/// lists them all.
const PREVIEW_LINES: usize = 10;

/// The toolbar's Presets button and its drop-down.
pub(crate) fn menu(ui: &mut Ui, texts: &UiText, state: &mut SelectState) {
    let startup = state
        .presets
        .on_start()
        .map(|preset| preset.name.clone())
        .unwrap_or_default();
    let label = if startup.is_empty() {
        icons::with_label(icons::PRESETS, texts.presets_menu)
    } else {
        icons::with_label(icons::PRESETS, &startup)
    };

    ui.menu_button(label, |ui| {
        ui.set_min_width(240.0);
        if state.presets.is_empty() {
            ui.label(
                RichText::new(texts.presets_none)
                    .size(theme::FONT_SMALL)
                    .color(theme::MUTED),
            );
        }
        // Collected first: applying one borrows the plan mutably.
        let mut apply = None;
        for (index, preset) in state.presets.as_slice().iter().enumerate() {
            let glyph = if preset.on_start {
                icons::PIN
            } else {
                icons::PRESET_ITEM
            };
            if ui
                .button(icons::with_label(glyph, &preset.name))
                .on_hover_ui(|ui| contents(ui, texts, &state.plan, &preset.keys, PREVIEW_LINES))
                .clicked()
            {
                apply = Some(index);
            }
        }
        if let Some(index) = apply
            && let Some(preset) = state.presets.get(index)
        {
            viewmodel::apply_saved_selection(&mut state.plan, &preset.keys, EmptyFolderRows::Check);
            ui.close();
        }

        ui.separator();
        if ui.button(texts.presets_save_current).clicked() {
            state.modal = Some(SelectModal::SavePreset {
                name: String::new(),
                error: None,
            });
            ui.close();
        }
        let manage = ui.add_enabled(
            !state.presets.is_empty(),
            egui::Button::new(texts.presets_manage),
        );
        if manage.clicked() {
            state.modal = Some(SelectModal::ManagePresets);
            ui.close();
        }
    })
    .response
    .on_hover_text(texts.presets_tooltip);
}

/// The Save dialog: name the current selection. Returns true when it closed.
pub(crate) fn save_dialog(
    ctx: &egui::Context,
    texts: &UiText,
    state: &mut SelectState,
    name: &mut String,
    error: &mut Option<PresetError>,
) -> bool {
    let selected = viewmodel::selected_keys(&state.plan);
    let mut close = false;
    let mut commit = false;
    let response = egui::Modal::new(egui::Id::unique("save_preset")).show(ctx, |ui| {
        ui.set_max_width(420.0);
        ui.label(
            RichText::new(texts.presets_save_title)
                .family(theme::bold())
                .size(theme::FONT_HEADING),
        );
        ui.separator();
        ui.label(
            RichText::new(texts.presets_save_count(selected.len()))
                .size(theme::FONT_SMALL)
                .color(theme::MUTED),
        );
        ui.add_space(theme::SPACE_SM);
        let field = ui.add(
            egui::TextEdit::singleline(name)
                .hint_text(texts.presets_name_hint)
                .desired_width(f32::INFINITY),
        );
        field.request_focus();
        if field.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
            commit = true;
        }
        // Saving over a name already in the list is how you update a preset,
        // so say that before the click rather than refusing it after.
        if state.presets.position(name).is_some() {
            ui.label(
                RichText::new(texts.presets_replace_note)
                    .size(theme::FONT_SMALL)
                    .color(theme::MUTED),
            );
        }
        if let Some(error) = *error {
            ui.label(RichText::new(error_text(texts, error)).color(theme::DANGER));
        }
        ui.separator();
        ui.horizontal(|ui| {
            if ui.button(texts.presets_save_action).clicked() {
                commit = true;
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button(texts.action_cancel).clicked() {
                    close = true;
                }
            });
        });
    });

    if commit {
        match state.presets.save(name, selected) {
            Ok(_) => close = true,
            Err(refusal) => *error = Some(refusal),
        }
    }
    close || response.should_close()
}

/// The Manage dialog: rename, pick the startup preset, see what each preset
/// holds, apply, update, or delete.
#[expect(
    clippy::too_many_lines,
    reason = "each preset's row and its contents read best in one place"
)]
pub(crate) fn manage_dialog(ctx: &egui::Context, texts: &UiText, state: &mut SelectState) -> bool {
    let mut close = false;
    let mut apply = None;
    let mut update = None;
    let mut remove = None;
    let mut startup: Option<(usize, bool)> = None;
    let mut renamed: Option<(usize, String)> = None;

    let response = egui::Modal::new(egui::Id::unique("manage_presets")).show(ctx, |ui| {
        ui.set_max_width(560.0);
        ui.label(
            RichText::new(texts.presets_manage_title)
                .family(theme::bold())
                .size(theme::FONT_HEADING),
        );
        ui.separator();
        ui.label(
            RichText::new(texts.presets_startup_note)
                .size(theme::FONT_SMALL)
                .color(theme::MUTED),
        );
        ui.add_space(theme::SPACE_SM);

        egui::ScrollArea::vertical()
            .max_height(340.0)
            .show(ui, |ui| {
                for (index, preset) in state.presets.as_slice().iter().enumerate() {
                    let mut name = preset.name.clone();
                    let mut on_start = preset.on_start;
                    components_row(ui, index, |ui| {
                        ui.vertical(|ui| {
                            ui.horizontal(|ui| {
                                if ui
                                    .add(
                                        egui::TextEdit::singleline(&mut name)
                                            .desired_width(200.0)
                                            .hint_text(texts.presets_name_hint),
                                    )
                                    .changed()
                                {
                                    renamed = Some((index, name.clone()));
                                }
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        if ui
                                            .button(icons::RECYCLE)
                                            .on_hover_text(texts.presets_delete)
                                            .clicked()
                                        {
                                            remove = Some(index);
                                        }
                                        if ui
                                            .button(texts.presets_update)
                                            .on_hover_text(texts.presets_update_hint)
                                            .clicked()
                                        {
                                            update = Some(index);
                                        }
                                        if ui.button(texts.presets_apply).clicked() {
                                            apply = Some(index);
                                        }
                                        if ui
                                            .checkbox(&mut on_start, texts.presets_on_start)
                                            .on_hover_text(texts.presets_on_start_hint)
                                            .changed()
                                        {
                                            startup = Some((index, on_start));
                                        }
                                    },
                                );
                            });
                            egui::CollapsingHeader::new(
                                RichText::new(texts.presets_target_count(preset.keys.len()))
                                    .size(theme::FONT_SMALL)
                                    .color(theme::MUTED),
                            )
                            .id_salt(("preset-contents", index))
                            .show(ui, |ui| {
                                contents(ui, texts, &state.plan, &preset.keys, usize::MAX);
                            });
                        });
                    });
                }
            });

        ui.separator();
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.button(texts.dialog_close).clicked() {
                close = true;
            }
        });
    });

    // Applied after the loop so the list is not borrowed while it changes.
    if let Some((index, name)) = renamed {
        // A refused rename leaves the stored name alone; the field catches up
        // on the next frame.
        let _ = state.presets.rename(index, &name);
    }
    if let Some((index, on)) = startup {
        state.presets.set_on_start(index, on);
    }
    if let Some(index) = apply
        && let Some(preset) = state.presets.get(index)
    {
        viewmodel::apply_saved_selection(&mut state.plan, &preset.keys, EmptyFolderRows::Check);
    }
    if let Some(index) = update
        && let Some(preset) = state.presets.get(index)
    {
        // Saving under the same name keeps the name and Use on start.
        let name = preset.name.clone();
        let _ = state
            .presets
            .save(&name, viewmodel::selected_keys(&state.plan));
    }
    if let Some(index) = remove {
        state.presets.remove(index);
    }
    if state.presets.is_empty() {
        close = true;
    }
    close || response.should_close()
}

/// What picking a preset does on this scan: how many of its targets it
/// checks, then up to `limit` of them, each one it leaves unchecked greyed
/// out with the reason.
fn contents(
    ui: &mut Ui,
    texts: &UiText,
    plan: &cleaner_core::Plan,
    keys: &[TargetKey],
    limit: usize,
) {
    let targets = viewmodel::preset_targets(plan, keys);
    let checks = targets
        .iter()
        .filter(|(_, status)| *status == PresetTargetStatus::Checks)
        .count();
    ui.label(
        RichText::new(texts.presets_apply_summary(checks, targets.len())).size(theme::FONT_SMALL),
    );
    for ((app, label), status) in targets.iter().take(limit) {
        let name = format!("{app} - {label}");
        let line = match status {
            PresetTargetStatus::Checks if app == cleaner_core::EMPTY_FOLDERS_APP => {
                RichText::new(format!("{name} ({})", texts.presets_status_folders_on_pick))
            }
            PresetTargetStatus::Checks => RichText::new(name),
            other => RichText::new(format!("{name} ({})", status_text(texts, *other)))
                .color(theme::MUTED),
        };
        ui.label(line.size(theme::FONT_SMALL));
    }
    if targets.len() > limit {
        ui.label(
            RichText::new(texts.and_more(targets.len() - limit))
                .size(theme::FONT_SMALL)
                .color(theme::MUTED),
        );
    }
}

fn status_text(texts: &UiText, status: PresetTargetStatus) -> &'static str {
    match status {
        PresetTargetStatus::Checks => "",
        PresetTargetStatus::Running => texts.presets_status_running,
        PresetTargetStatus::NothingNow => texts.presets_status_nothing_now,
        PresetTargetStatus::NotFound => texts.presets_status_not_found,
        PresetTargetStatus::NoEmptyFolders => texts.presets_status_no_empty_folders,
        PresetTargetStatus::Gone => texts.presets_status_gone,
    }
}

/// One zebra-striped preset row inside the Manage dialog.
fn components_row(ui: &mut Ui, index: usize, add: impl FnOnce(&mut Ui)) {
    crate::ui::components::striped_row(ui, index % 2 == 1, add);
}

fn error_text(texts: &UiText, error: PresetError) -> String {
    match error {
        PresetError::EmptyName => texts.presets_error_empty.to_owned(),
        PresetError::DuplicateName => texts.presets_error_duplicate.to_owned(),
        PresetError::Full => texts.presets_error_full(MAX_PRESETS),
    }
}
