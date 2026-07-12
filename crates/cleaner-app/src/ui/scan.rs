//! The scanning screen: a centered spinner, title, progress bar, and status.

use eframe::egui::{self, RichText, Ui};

use crate::strings::UiText;
use crate::theme;

pub(crate) fn show(ui: &mut Ui, texts: &UiText, fraction: f32, status: &str) {
    ui.add_space(ui.available_height() * 0.3);
    ui.vertical_centered(|ui| {
        ui.add(egui::Spinner::new().size(28.0));
        ui.add_space(theme::SPACE_MD);
        ui.label(
            RichText::new(texts.cache_scan_card_title)
                .family(theme::bold())
                .size(theme::FONT_DISPLAY),
        );
        ui.label(RichText::new(texts.cache_scan_card_subtitle).color(theme::MUTED));
        ui.add_space(theme::SPACE_MD);
        ui.add(
            egui::ProgressBar::new(fraction)
                .desired_width(440.0)
                .desired_height(8.0),
        );
        ui.add_space(theme::SPACE_SM);
        ui.label(RichText::new(status).color(theme::MUTED).small());
    });
}
