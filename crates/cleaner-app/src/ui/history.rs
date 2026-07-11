//! The history overlay: aggregate stats, the run list, and the details pane.

use cleaner_core::{ExecResult, human_bytes};
use eframe::egui::{self, Color32, RichText, Ui};

use crate::app::HistoryState;
use crate::strings::UiText;
use crate::theme;
use crate::ui::components;
use crate::viewmodel::{
    format_duration, format_timestamp, run_details_text, stats_summary_text, summary_line,
};

pub(crate) enum HistoryAction {
    Refresh,
}

pub(crate) fn show(ui: &mut Ui, texts: &UiText, state: &mut HistoryState) -> Option<HistoryAction> {
    let mut action = None;

    if let Some(error) = &state.error {
        components::centered_status(ui, error);
        return None;
    }
    if !state.loaded {
        ui.centered_and_justified(|ui| {
            ui.add(egui::Spinner::new().size(28.0));
        });
        return None;
    }

    ui.horizontal(|ui| {
        ui.label(stats_summary_text(
            texts,
            &state.runs,
            state.skipped,
            jiff::Timestamp::now(),
        ));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
            if ui.button(texts.action_refresh).clicked() {
                action = Some(HistoryAction::Refresh);
            }
        });
    });
    ui.add_space(6.0);

    if state.runs.is_empty() {
        let message = if state.skipped > 0 {
            format!(
                "{} {} files could not be read.",
                texts.history_no_runs, state.skipped
            )
        } else {
            texts.history_no_runs.to_owned()
        };
        components::centered_status(ui, &message);
        return action;
    }

    components::titled_card(
        ui,
        texts.history_previous_runs_title,
        texts.history_previous_runs_subtitle,
        |ui| {
            let list_width = ui.available_width() * 0.36;
            let height = ui.available_height();
            ui.horizontal_top(|ui| {
                ui.vertical(|ui| {
                    ui.set_width(list_width);
                    egui::ScrollArea::vertical()
                        .id_salt("history-list")
                        .max_height(height)
                        .auto_shrink([false, true])
                        .show(ui, |ui| {
                            run_list(ui, texts, state);
                        });
                });
                ui.separator();
                ui.vertical(|ui| {
                    egui::ScrollArea::vertical()
                        .id_salt("history-details")
                        .max_height(height)
                        .auto_shrink([false, true])
                        .show(ui, |ui| {
                            let selected = state.selected.min(state.runs.len() - 1);
                            ui.label(run_details_text(texts, &state.runs[selected]));
                        });
                });
            });
        },
    );
    action
}

fn run_list(ui: &mut Ui, texts: &UiText, state: &mut HistoryState) {
    for (index, run) in state.runs.iter().enumerate() {
        let fill = if index == state.selected {
            theme::SELECTION
        } else if index % 2 == 1 {
            theme::ROW_ALT
        } else {
            Color32::TRANSPARENT
        };
        let response = egui::Frame::new()
            .fill(fill)
            .corner_radius(6)
            .inner_margin(egui::Margin::symmetric(8, 6))
            .show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                run_row(ui, texts, run);
            })
            .response;
        if response.interact(egui::Sense::click()).clicked() {
            state.selected = index;
        }
    }
}

fn run_row(ui: &mut Ui, texts: &UiText, run: &ExecResult) {
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.label(RichText::new(format_timestamp(run.run_timestamp())).family(theme::bold()));
            ui.label(
                RichText::new(summary_line(&[
                    &texts.items_count(run.total_selected),
                    &format_duration(run.duration_ms),
                ]))
                .color(theme::MUTED)
                .small(),
            );
        });
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.vertical(|ui| {
                ui.with_layout(egui::Layout::top_down(egui::Align::Max), |ui| {
                    ui.label(RichText::new(human_bytes(run.total_bytes)).family(theme::bold()));
                    if run.error_count > 0 {
                        ui.label(
                            RichText::new(texts.errors_count(run.error_count))
                                .color(theme::DANGER)
                                .small(),
                        );
                    } else {
                        ui.label(
                            RichText::new(texts.result_status_ok)
                                .color(theme::MUTED)
                                .small(),
                        );
                    }
                });
            });
        });
    });
}
