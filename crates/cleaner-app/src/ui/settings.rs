//! The Settings modal: what the app remembers between launches.
//!
//! Preview-only mode and empty-folder removal are not settings. Both stay per
//! run by the safety model, and the note at the bottom says so.

use eframe::egui::{self, RichText};

use crate::strings::UiText;
use crate::theme;

pub(crate) enum SettingsAction {
    /// Drop the stored selection and rescan, so the catalog defaults decide
    /// again.
    ResetSelection,
}

pub(crate) fn show(
    ctx: &egui::Context,
    texts: &UiText,
    remember_selection: &mut bool,
    show_empty: &mut bool,
    open: &mut bool,
) -> Option<SettingsAction> {
    if !*open {
        return None;
    }
    let mut action = None;
    let mut close = false;
    let response = egui::Modal::new(egui::Id::new("settings")).show(ctx, |ui| {
        ui.set_max_width(460.0);
        ui.label(
            RichText::new(texts.settings_title)
                .family(theme::bold())
                .size(theme::FONT_HEADING),
        );
        ui.separator();

        toggle(
            ui,
            remember_selection,
            texts.settings_remember,
            texts.settings_remember_hint,
        );
        ui.add_space(theme::SPACE_SM);
        toggle(
            ui,
            show_empty,
            texts.settings_show_empty,
            texts.settings_show_empty_hint,
        );

        ui.add_space(theme::SPACE_SM);
        ui.label(
            RichText::new(texts.settings_safety_note)
                .size(theme::FONT_SMALL)
                .color(theme::MUTED),
        );
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

/// One checkbox with a muted line of explanation under it.
fn toggle(ui: &mut egui::Ui, value: &mut bool, label: &str, hint: &str) {
    ui.checkbox(value, label);
    ui.label(
        RichText::new(hint)
            .size(theme::FONT_SMALL)
            .color(theme::MUTED),
    );
}
