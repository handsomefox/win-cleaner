//! The Presets menu in the selection toolbar, plus the Save and Manage
//! dialogs behind it.
//!
//! Applying a preset goes through [`viewmodel::apply_saved_selection`], the
//! same path a remembered selection takes, so the empty-target and
//! empty-folder rules hold for a preset too.

use eframe::egui::{self, RichText, Ui};

use crate::app::{SelectModal, SelectState};
use crate::icons;
use crate::presets::{MAX_PRESETS, PresetError};
use crate::strings::UiText;
use crate::theme;
use crate::viewmodel;

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
        let entries: Vec<(usize, String, bool, usize)> = state
            .presets
            .as_slice()
            .iter()
            .enumerate()
            .map(|(index, preset)| {
                (
                    index,
                    preset.name.clone(),
                    preset.on_start,
                    preset.keys.len(),
                )
            })
            .collect();
        let mut apply = None;
        for (index, name, on_start, targets) in entries {
            let glyph = if on_start {
                icons::PIN
            } else {
                icons::PRESET_ITEM
            };
            if ui
                .button(icons::with_label(glyph, &name))
                .on_hover_text(texts.presets_apply_hint(targets))
                .clicked()
            {
                apply = Some(index);
            }
        }
        if let Some(index) = apply
            && let Some(preset) = state.presets.get(index)
        {
            viewmodel::apply_saved_selection(&mut state.plan, &preset.keys);
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
    let response = egui::Modal::new(egui::Id::new("save_preset")).show(ctx, |ui| {
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

/// The Manage dialog: rename, pick the startup preset, apply, or delete.
pub(crate) fn manage_dialog(ctx: &egui::Context, texts: &UiText, state: &mut SelectState) -> bool {
    let mut close = false;
    let mut apply = None;
    let mut remove = None;
    let mut startup: Option<(usize, bool)> = None;
    let mut renamed: Option<(usize, String)> = None;

    let response = egui::Modal::new(egui::Id::new("manage_presets")).show(ctx, |ui| {
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
                        ui.label(
                            RichText::new(texts.presets_target_count(preset.keys.len()))
                                .size(theme::FONT_SMALL)
                                .color(theme::MUTED),
                        );
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui
                                .button(icons::RECYCLE)
                                .on_hover_text(texts.presets_delete)
                                .clicked()
                            {
                                remove = Some(index);
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
        viewmodel::apply_saved_selection(&mut state.plan, &preset.keys);
    }
    if let Some(index) = remove {
        state.presets.remove(index);
    }
    if state.presets.is_empty() {
        close = true;
    }
    close || response.should_close()
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
