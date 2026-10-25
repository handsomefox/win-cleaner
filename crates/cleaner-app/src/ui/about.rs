//! The About modal: icon, version, credits, project link, and shortcuts to
//! the diagnostics log folder and the app updates modal.

use eframe::egui::{self, RichText};

use crate::icons;
use crate::strings::UiText;
use crate::theme;

/// Returns true when the user asked for the app updates modal.
pub(crate) fn show(ctx: &egui::Context, texts: &UiText, open: &mut bool) -> bool {
    if !*open {
        return false;
    }
    let mut close = false;
    let mut updates = false;
    let response = egui::Modal::new(egui::Id::new("about")).show(ctx, |ui| {
        ui.set_max_width(440.0);
        ui.horizontal(|ui| {
            ui.add(
                egui::Image::new(egui::include_image!("../../assets/icon.png"))
                    .fit_to_exact_size(egui::vec2(64.0, 64.0)),
            );
            ui.vertical(|ui| {
                ui.label(
                    RichText::new(texts.app_title)
                        .family(theme::bold())
                        .size(theme::FONT_HEADING),
                );
                ui.label(
                    RichText::new(format!(
                        "{} {}",
                        texts.about_version_label,
                        env!("CARGO_PKG_VERSION")
                    ))
                    .color(theme::MUTED),
                );
                ui.label(RichText::new(texts.about_fonts).color(theme::MUTED));
                ui.hyperlink_to(texts.about_repo_label, texts.about_repo_url);
            });
        });
        ui.separator();
        ui.horizontal(|ui| {
            if ui.button(texts.about_open_log).clicked() {
                open_log_folder();
            }
            if ui
                .button(icons::with_label(icons::UPDATE, texts.about_updates))
                .on_hover_text(texts.about_updates_hint)
                .clicked()
            {
                updates = true;
                close = true;
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
    updates
}

/// Opens the diagnostics log directory in the system file manager, so the
/// user can grab the log file when reporting a problem.
fn open_log_folder() {
    let Some(dir) = cleaner_platform::logs_dir() else {
        return;
    };
    if std::fs::create_dir_all(&dir).is_err() {
        return;
    }
    if let Err(err) = cleaner_platform::open_folder(&dir) {
        tracing::warn!("failed to open log folder: {err}");
    }
}
