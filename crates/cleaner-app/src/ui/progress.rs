//! The delete-progress screen: percent, live path, count, and a stats strip.

use cleaner_core::human_bytes;
use eframe::egui::{self, RichText, Ui};

use crate::app::DeletingState;
use crate::icons;
use crate::strings::UiText;
use crate::theme;
use crate::ui::components;

pub(crate) fn show(ui: &mut Ui, texts: &UiText, state: &DeletingState) {
    components::titled_card(
        ui,
        texts.cache_delete_card_title,
        texts.cache_delete_card_subtitle,
        |ui| {
            let fraction = components::fraction(state.current, state.total);

            ui.add_space(theme::SPACE_XS);
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(texts.progress_moving_title)
                        .family(theme::bold())
                        .size(theme::FONT_BODY),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(
                        RichText::new(format!("{:.0}%", f64::from(fraction) * 100.0))
                            .family(theme::bold())
                            .size(theme::FONT_DISPLAY),
                    );
                });
            });
            ui.label(RichText::new(&state.message).color(theme::MUTED));
            ui.add_space(theme::SPACE_SM);
            ui.add(egui::ProgressBar::new(fraction).desired_height(10.0));
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(format!(
                        "{} / {} {}",
                        state.current, state.total, texts.unit_items
                    ))
                    .family(theme::bold()),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(
                        RichText::new(icons::with_label(icons::RECYCLE, texts.recycle_bin))
                            .color(theme::MUTED),
                    );
                });
            });
            ui.separator();
            ui.horizontal(|ui| {
                components::stat_chip(
                    ui,
                    texts.stat_selected,
                    &texts.items_count(state.selected_items),
                );
                components::stat_chip(ui, texts.stat_scope, &human_bytes(state.selected_bytes));
                components::stat_chip(ui, texts.stat_destination, texts.recycle_bin);
            });
        },
    );
}
