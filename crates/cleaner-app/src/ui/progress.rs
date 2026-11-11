//! The delete-progress screen: a centered column with the progress bar, the
//! run totals, and the live path being moved.

use cleaner_core::human_bytes;
use eframe::egui::{self, RichText, Ui};

use crate::app::DeletingState;
use crate::strings::UiText;
use crate::theme;
use crate::ui::components;

/// Draws the screen and reports whether the user pressed Stop.
pub(crate) fn show(ui: &mut Ui, texts: &UiText, state: &DeletingState) -> bool {
    let mut stop = false;
    let fraction = components::fraction(state.current, state.total);
    ui.add_space(ui.available_height() * 0.3);
    ui.vertical_centered(|ui| {
        ui.label(
            RichText::new(texts.cache_delete_card_title)
                .family(theme::bold())
                .size(theme::FONT_DISPLAY),
        );
        ui.label(RichText::new(texts.cache_delete_card_subtitle).color(theme::MUTED));
        ui.add_space(theme::SPACE_MD);
        ui.add(
            egui::ProgressBar::new(fraction)
                .desired_width(440.0)
                .desired_height(10.0),
        );
        ui.add_space(theme::SPACE_XS);
        ui.label(format!(
            "{} / {} {}  ·  {}",
            state.current,
            state.total,
            texts.unit_items,
            human_bytes(state.selected_bytes)
        ));
        ui.add_space(theme::SPACE_SM);
        ui.label(RichText::new(&state.message).color(theme::MUTED).small());
        ui.add_space(theme::SPACE_MD);
        if state.stopping {
            ui.label(RichText::new(texts.cleanup_stopping).color(theme::MUTED));
        } else if ui
            .button(texts.action_stop)
            .on_hover_text(texts.action_stop_hint)
            .clicked()
        {
            stop = true;
        }
    });
    stop
}
