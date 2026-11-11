//! The Settings modal: what the app remembers between launches.
//!
//! Preview-only mode and empty-folder removal are not settings. Both stay per
//! run by the safety model, and the note at the bottom says so.

use cleaner_core::Rules;
use eframe::egui::{self, RichText};

use crate::strings::UiText;
use crate::theme;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SettingsAction {
    /// Drop the stored selection and rescan, so the catalog defaults decide
    /// again.
    ResetSelection,
    AddFolder,
    AddFile,
    RemoveExclusion(usize),
    Export,
    Import,
}

/// What the dialog edits in place.
pub(crate) struct Values<'a> {
    pub remember_selection: &'a mut bool,
    pub show_empty: &'a mut bool,
    pub rules: &'a mut Rules,
    pub always_elevated: &'a mut bool,
}

#[expect(
    clippy::too_many_lines,
    reason = "one dialog laid out top to bottom reads best in one place"
)]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Values only bundles mutable borrows of the caller's state"
)]
pub(crate) fn show(
    ctx: &egui::Context,
    texts: &UiText,
    values: Values<'_>,
    open: &mut bool,
) -> Option<SettingsAction> {
    if !*open {
        return None;
    }
    let mut action = None;
    let mut close = false;
    let response = egui::Modal::new(egui::Id::unique("settings")).show(ctx, |ui| {
        ui.set_max_width(560.0);
        ui.label(
            RichText::new(texts.settings_title)
                .family(theme::bold())
                .size(theme::FONT_HEADING),
        );
        ui.separator();
        egui::ScrollArea::vertical()
            .max_height(ctx.content_rect().height() * 0.7)
            .show(ui, |ui| {
                toggle(
                    ui,
                    values.remember_selection,
                    texts.settings_remember,
                    texts.settings_remember_hint,
                );
                ui.add_space(theme::SPACE_SM);
                toggle(
                    ui,
                    values.show_empty,
                    texts.settings_show_empty,
                    texts.settings_show_empty_hint,
                );

                section(ui, texts.settings_rules_title);
                toggle(
                    ui,
                    &mut values.rules.skip_running_apps,
                    texts.settings_skip_running,
                    texts.settings_skip_running_hint,
                );
                ui.add_space(theme::SPACE_SM);
                ui.horizontal(|ui| {
                    ui.label(texts.settings_min_age);
                    ui.add(
                        egui::DragValue::new(&mut values.rules.min_age_days)
                            .range(0..=3650)
                            .suffix(texts.settings_days_suffix),
                    );
                });
                let mut kilobytes = values.rules.min_size_bytes / 1024;
                ui.horizontal(|ui| {
                    ui.label(texts.settings_min_size);
                    if ui
                        .add(
                            egui::DragValue::new(&mut kilobytes)
                                .range(0..=10 * 1024 * 1024)
                                .suffix(texts.settings_kb_suffix),
                        )
                        .changed()
                    {
                        values.rules.min_size_bytes = kilobytes * 1024;
                    }
                });
                ui.label(
                    RichText::new(texts.settings_limits_hint)
                        .size(theme::FONT_SMALL)
                        .color(theme::MUTED),
                );

                section(ui, texts.settings_exclusions_title);
                if values.rules.exclusions.is_empty() {
                    ui.label(RichText::new(texts.settings_exclusions_none).color(theme::MUTED));
                }
                for (index, path) in values.rules.exclusions.iter().enumerate() {
                    ui.horizontal(|ui| {
                        if ui.small_button(texts.settings_remove).clicked() {
                            action = Some(SettingsAction::RemoveExclusion(index));
                        }
                        ui.add(egui::Label::new(path.display().to_string()).truncate());
                    });
                }
                ui.horizontal(|ui| {
                    if ui.button(texts.settings_add_folder).clicked() {
                        action = Some(SettingsAction::AddFolder);
                    }
                    if ui.button(texts.settings_add_file).clicked() {
                        action = Some(SettingsAction::AddFile);
                    }
                });
                ui.label(
                    RichText::new(texts.settings_exclusions_hint)
                        .size(theme::FONT_SMALL)
                        .color(theme::MUTED),
                );

                if cfg!(windows) {
                    section(ui, texts.settings_admin_title);
                    toggle(
                        ui,
                        values.always_elevated,
                        texts.settings_always_elevated,
                        texts.settings_always_elevated_hint,
                    );
                }

                section(ui, texts.settings_file_title);
                ui.horizontal(|ui| {
                    if ui.button(texts.settings_export).clicked() {
                        action = Some(SettingsAction::Export);
                    }
                    if ui.button(texts.settings_import).clicked() {
                        action = Some(SettingsAction::Import);
                    }
                });
                ui.label(
                    RichText::new(texts.settings_file_hint)
                        .size(theme::FONT_SMALL)
                        .color(theme::MUTED),
                );

                ui.add_space(theme::SPACE_SM);
                ui.label(
                    RichText::new(texts.settings_safety_note)
                        .size(theme::FONT_SMALL)
                        .color(theme::MUTED),
                );
            });
        ui.separator();
        ui.horizontal(|ui| {
            if ui
                .button(texts.settings_reset)
                .on_hover_text(texts.settings_reset_hint)
                .clicked()
            {
                action = Some(SettingsAction::ResetSelection);
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button(texts.dialog_close).clicked() {
                    close = true;
                }
            });
        });
    });
    if close || response.should_close() {
        *open = false;
    }
    action
}

/// A bold heading that starts a group of settings.
fn section(ui: &mut egui::Ui, title: &str) {
    ui.add_space(theme::SPACE_MD);
    ui.label(RichText::new(title).family(theme::bold()));
    ui.add_space(theme::SPACE_XS);
}

/// One checkbox with a muted line of explanation under it.
fn toggle(ui: &mut egui::Ui, value: &mut bool, label: &str, hint: &str) {
    ui.checkbox(value, label);
    ui.label(
        RichText::new(hint)
            .size(theme::FONT_SMALL)
            .color(theme::MUTED),
    );
}
