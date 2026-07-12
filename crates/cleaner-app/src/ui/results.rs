//! The results screen: headline, error summary, per-group outcomes, and the
//! History/Done actions.

use cleaner_core::{ExecResult, GroupResult, human_bytes};
use eframe::egui::{self, RichText, Ui};

use crate::app::{ResultsModal, ResultsState};
use crate::strings::UiText;
use crate::theme;
use crate::ui::components;
use crate::viewmodel::cleanup_result_summary;

pub(crate) enum ResultsAction {
    Done,
    History,
}

pub(crate) fn show(ui: &mut Ui, texts: &UiText, state: &mut ResultsState) -> Option<ResultsAction> {
    let mut action = None;
    let (headline, summary) = cleanup_result_summary(texts, &state.result, false);

    let mut open_errors = false;
    let mut open_group: Option<usize> = None;
    components::titled_card(ui, &headline, &summary, |ui| {
        if let Some(stats_error) = &state.stats_error {
            ui.label(
                RichText::new(format!("Failed to write stats: {stats_error}")).color(theme::DANGER),
            );
        }
        if let Some((error_summary, _)) = build_error_summary(texts, &state.result) {
            ui.label(&error_summary);
            if ui.button(texts.result_error_details).clicked() {
                open_errors = true;
            }
            ui.separator();
        }
        egui::ScrollArea::vertical()
            .auto_shrink([false, true])
            .show(ui, |ui| {
                if state.result.groups.is_empty() {
                    components::centered_status(ui, texts.result_no_group_details);
                    return;
                }
                for (index, group) in state.result.groups.iter().enumerate() {
                    components::tree_row(ui, 0, index % 2 == 1, |ui| {
                        ui.label(RichText::new(&group.app).family(theme::bold()));
                        ui.label(&group.label);
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.small_button(texts.result_details).clicked() {
                                open_group = Some(index);
                            }
                            ui.label(group_status(texts, group));
                            ui.label(human_bytes(group.bytes));
                        });
                    });
                }
            });
    });

    ui.add_space(theme::SPACE_SM);
    ui.horizontal(|ui| {
        if ui.button(texts.action_history).clicked() {
            action = Some(ResultsAction::History);
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui
                .add(components::accent_button(texts.action_done))
                .clicked()
            {
                action = Some(ResultsAction::Done);
            }
        });
    });

    if open_errors {
        state.modal = Some(ResultsModal::Errors);
    }
    if let Some(index) = open_group {
        state.modal = Some(ResultsModal::GroupDetails(index));
    }
    modals(ui.ctx(), texts, state);
    action
}

fn group_status(texts: &UiText, group: &GroupResult) -> RichText {
    if group.paths_failed > 0 {
        RichText::new(texts.failed_count(group.paths_failed)).color(theme::DANGER)
    } else if group.paths_attempted == 0 {
        RichText::new(texts.result_status_skipped).color(theme::MUTED)
    } else {
        RichText::new(texts.result_status_ok).color(theme::MUTED)
    }
}

fn modals(ctx: &egui::Context, texts: &UiText, state: &mut ResultsState) {
    match state.modal {
        None => {}
        Some(ResultsModal::Errors) => {
            let Some((_, details)) = build_error_summary(texts, &state.result) else {
                state.modal = None;
                return;
            };
            if components::text_modal(
                ctx,
                "cleanup-errors",
                texts.dialog_cleanup_errors,
                &details,
                texts.dialog_close,
            ) {
                state.modal = None;
            }
        }
        Some(ResultsModal::GroupDetails(index)) => {
            let Some(group) = state.result.groups.get(index) else {
                state.modal = None;
                return;
            };
            let body = result_group_details_text(group);
            if components::text_modal(
                ctx,
                "cleanup-group-details",
                texts.dialog_cleanup_details_title,
                &body,
                texts.dialog_close,
            ) {
                state.modal = None;
            }
        }
    }
}

/// Short error summary plus the full error details text, or `None` when the
/// run had no errors.
fn build_error_summary(texts: &UiText, result: &ExecResult) -> Option<(String, String)> {
    use std::fmt::Write as _;

    if result.error_count == 0 {
        return None;
    }
    let mut group_names = Vec::new();
    let mut details = String::new();
    let _ = writeln!(
        details,
        "{} {}\n",
        texts.run_label_errors, result.error_count
    );
    for group in &result.groups {
        if group.errors.is_empty() {
            continue;
        }
        group_names.push(format!("{} - {}", group.app, group.label));
        let _ = writeln!(
            details,
            "{} - {} ({})",
            group.app,
            group.label,
            texts.issues_count(group.errors.len())
        );
        for error in &group.errors {
            let _ = writeln!(details, "- {}: {}", error.path, error.error);
        }
        details.push('\n');
    }
    let limit = group_names.len().min(3);
    let mut summary = format!(
        "Errors in {}: {}",
        texts.items_count(group_names.len()),
        group_names[..limit].join(", ")
    );
    if group_names.len() > limit {
        summary.push_str(", …");
    }
    Some((summary, details))
}

fn result_group_details_text(group: &GroupResult) -> String {
    use std::fmt::Write as _;

    let mut out = String::new();
    let _ = writeln!(
        out,
        "{} - {}\nEstimated size: {}\nPaths attempted: {}\nPaths failed: {}",
        group.app,
        group.label,
        human_bytes(group.bytes),
        group.paths_attempted,
        group.paths_failed
    );
    if !group.errors.is_empty() {
        out.push_str("\nErrors:\n");
        for error in &group.errors {
            let _ = writeln!(out, "- {}: {}", error.path, error.error);
        }
    }
    out
}
