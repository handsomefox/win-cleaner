//! The history view: aggregate stat chips, the run list, and a structured
//! per-run details pane. Drawn in the main pane so the sidebar stays visible.

use cleaner_core::{ExecResult, human_bytes};
use eframe::egui::{self, Color32, RichText, Ui};

use crate::app::HistoryState;
use crate::icons;
use crate::strings::UiText;
use crate::theme;
use crate::ui::components;
use crate::viewmodel::{
    format_duration, format_timestamp, format_timestamp_long, stats_totals, summary_line,
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
        if !state.runs.is_empty() {
            let totals = stats_totals(state.runs.iter(), jiff::Timestamp::now());
            components::stat_chip(ui, texts.stats_runs_label, &state.runs.len().to_string());
            components::stat_chip(
                ui,
                texts.stats_total_freed_label,
                &human_bytes(totals.total),
            );
            components::stat_chip(ui, texts.stats_last7_label, &human_bytes(totals.last7));
            components::stat_chip(ui, texts.stats_last30_label, &human_bytes(totals.last30));
        }
        if state.skipped > 0 {
            ui.label(
                RichText::new(format!("{} {}", state.skipped, texts.stats_unreadable))
                    .color(theme::MUTED)
                    .small(),
            );
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
            if ui
                .button(icons::with_label(icons::RESCAN, texts.action_refresh))
                .clicked()
            {
                action = Some(HistoryAction::Refresh);
            }
        });
    });
    ui.add_space(theme::SPACE_SM);

    if state.runs.is_empty() {
        components::centered_status(ui, texts.history_no_runs);
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
                            run_details(ui, texts, &state.runs[selected]);
                        });
                });
            });
        },
    );
    action
}

/// The structured details pane for one run: timestamp heading, stat chips,
/// then per-group outcome rows with their error lines.
fn run_details(ui: &mut Ui, texts: &UiText, run: &ExecResult) {
    ui.label(
        RichText::new(format_timestamp_long(run.run_timestamp()))
            .family(theme::bold())
            .size(theme::FONT_HEADING),
    );
    ui.add_space(theme::SPACE_XS);
    ui.horizontal(|ui| {
        components::stat_chip(ui, texts.run_label_items, &run.total_selected.to_string());
        components::stat_chip(ui, texts.run_label_freed, &human_bytes(run.total_bytes));
        components::stat_chip(
            ui,
            texts.run_label_duration,
            &format_duration(run.duration_ms),
        );
        error_chip(ui, texts, run.error_count);
    });
    ui.separator();

    if run.groups.is_empty() {
        ui.label(RichText::new(texts.result_no_group_details).color(theme::MUTED));
        return;
    }
    for (index, group) in run.groups.iter().enumerate() {
        components::striped_row(ui, index % 2 == 1, |ui| {
            ui.label(RichText::new(&group.app).family(theme::bold()));
            ui.label(&group.label);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(components::group_status(texts, group));
                ui.label(human_bytes(group.bytes));
            });
        });
        for error in &group.errors {
            ui.label(
                RichText::new(format!("{}: {}", error.path, error.error))
                    .color(theme::DANGER)
                    .small(),
            );
        }
    }
}

/// The error-count chip; the value turns danger-red when a run had failures.
fn error_chip(ui: &mut Ui, texts: &UiText, count: usize) {
    let color = if count > 0 {
        theme::DANGER
    } else {
        theme::TEXT
    };
    components::chip_frame().show(ui, |ui| {
        ui.vertical(|ui| {
            ui.label(
                RichText::new(count.to_string())
                    .family(theme::bold())
                    .color(color),
            );
            ui.label(
                RichText::new(texts.run_label_errors)
                    .color(theme::MUTED)
                    .small(),
            );
        });
    });
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
            .corner_radius(theme::RADIUS_SM)
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
                            RichText::new(icons::with_label(
                                icons::ERROR,
                                &texts.errors_count(run.error_count),
                            ))
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
