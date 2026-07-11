//! The scanning screen: a card with a spinner, status line, and progress bar.

use eframe::egui::{self, Ui};

use crate::strings::UiText;
use crate::ui::components;

pub(crate) fn show(ui: &mut Ui, texts: &UiText, fraction: f32, status: &str) {
    components::titled_card(
        ui,
        texts.cache_scan_card_title,
        texts.cache_scan_card_subtitle,
        |ui| {
            ui.add_space(12.0);
            ui.vertical_centered(|ui| {
                ui.add(egui::Spinner::new().size(28.0));
                ui.add_space(8.0);
                ui.label(status);
            });
            ui.add_space(12.0);
            ui.add(egui::ProgressBar::new(fraction).desired_height(8.0));
            ui.add_space(4.0);
        },
    );
}
