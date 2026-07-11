//! Shared building blocks: surface cards, chips, tree rows, size cells, and
//! plain-text modals shared across the application screens.

use cleaner_core::{Group, human_bytes};
use eframe::egui::{self, Color32, RichText, Ui};

use crate::strings::UiText;
use crate::theme;

/// Per-level left indent for the cache tree.
pub(crate) const INDENT_STEP: f32 = 32.0;

/// Rounded, bordered surface panel — the building block behind every card.
pub(crate) fn surface_frame() -> egui::Frame {
    egui::Frame::new()
        .fill(theme::SURFACE)
        .stroke(egui::Stroke::new(1.0, theme::BORDER))
        .corner_radius(14)
        .inner_margin(12)
}

/// Compact rounded pill used for header stats and progress figures.
pub(crate) fn chip_frame() -> egui::Frame {
    egui::Frame::new()
        .fill(theme::SURFACE_RAISED)
        .stroke(egui::Stroke::new(1.0, theme::BORDER))
        .corner_radius(9)
        .inner_margin(egui::Margin::symmetric(10, 6))
}

/// A surface card with a bold title, muted subtitle, and body.
pub(crate) fn titled_card(ui: &mut Ui, title: &str, subtitle: &str, body: impl FnOnce(&mut Ui)) {
    surface_frame().show(ui, |ui| {
        ui.set_min_width(ui.available_width());
        ui.label(RichText::new(title).family(theme::bold()).size(16.0));
        if !subtitle.is_empty() {
            ui.label(RichText::new(subtitle).color(theme::MUTED));
        }
        ui.separator();
        body(ui);
    });
}

/// Centers a muted status message in the remaining space.
pub(crate) fn centered_status(ui: &mut Ui, text: &str) {
    ui.centered_and_justified(|ui| {
        ui.label(RichText::new(text).color(theme::MUTED));
    });
}

/// A right-aligned size value tinted by its magnitude; zero renders as an
/// italic "Not found".
pub(crate) fn size_text(texts: &UiText, bytes: u64) -> RichText {
    if bytes == 0 {
        RichText::new(texts.not_found).color(theme::MUTED).italics()
    } else {
        RichText::new(human_bytes(bytes))
            .color(theme::magnitude_color(bytes))
            .family(theme::bold())
    }
}

/// Tri-state selection glyph for category and app header rows.
pub(crate) fn selection_glyph(selected: usize, total: usize) -> &'static str {
    if total == 0 || selected == 0 {
        "☐"
    } else if selected == total {
        "☑"
    } else {
        "◩"
    }
}

pub(crate) fn expand_chevron(expanded: bool) -> &'static str {
    if expanded { "▾" } else { "▸" }
}

/// A borderless button, used for glyphs and tappable tree headers.
pub(crate) fn flat_button(ui: &mut Ui, text: impl Into<egui::WidgetText>) -> egui::Response {
    ui.add(egui::Button::new(text).frame(false))
}

/// One tree row: an optional zebra stripe behind an indented horizontal
/// cluster spanning the full width.
pub(crate) fn tree_row(ui: &mut Ui, level: u8, striped: bool, add: impl FnOnce(&mut Ui)) {
    let fill = if striped {
        theme::ROW_ALT
    } else {
        Color32::TRANSPARENT
    };
    egui::Frame::new()
        .fill(fill)
        .inner_margin(egui::Margin::symmetric(6, 4))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.add_space(f32::from(level) * INDENT_STEP);
                add(ui);
            });
        });
}

/// A labeled value pill for the delete-progress stats strip.
pub(crate) fn stat_chip(ui: &mut Ui, label: &str, value: &str) {
    chip_frame().show(ui, |ui| {
        ui.vertical(|ui| {
            ui.label(RichText::new(value).family(theme::bold()));
            ui.label(RichText::new(label).color(theme::MUTED).small());
        });
    });
}

/// Safe progress fraction for `current` of `total`.
pub(crate) fn fraction(current: usize, total: usize) -> f32 {
    if total == 0 {
        return 0.0;
    }
    #[expect(clippy::cast_precision_loss, reason = "display-only approximation")]
    let value = current as f64 / total as f64;
    #[expect(clippy::cast_possible_truncation, reason = "clamped to [0, 1]")]
    {
        value.clamp(0.0, 1.0) as f32
    }
}

/// Shows a modal with a bold title, scrollable body text, and a close button.
/// Returns `true` when the modal asked to close this frame.
pub(crate) fn text_modal(
    ctx: &egui::Context,
    id: &str,
    title: &str,
    body: &str,
    close: &str,
) -> bool {
    let mut close_clicked = false;
    let response = egui::Modal::new(egui::Id::new(id)).show(ctx, |ui| {
        ui.set_max_width(720.0);
        ui.label(RichText::new(title).family(theme::bold()).size(16.0));
        ui.separator();
        egui::ScrollArea::vertical()
            .max_height(400.0)
            .show(ui, |ui| {
                ui.label(body);
            });
        ui.separator();
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.button(close).clicked() {
                close_clicked = true;
            }
        });
    });
    close_clicked || response.should_close()
}

/// The plain-text detail body for one plan group (paths + scan issues).
pub(crate) fn group_details_text(texts: &UiText, group: &Group) -> String {
    use std::fmt::Write as _;

    let mut out = String::new();
    let _ = writeln!(
        out,
        "App: {}\nTarget: {}\nEstimated size: {}\n\nPaths:",
        group.app,
        group.label,
        human_bytes(group.bytes)
    );
    if group.paths.is_empty() {
        let _ = writeln!(out, "- {}", texts.result_no_matching_paths);
    }
    for path in &group.paths {
        let _ = writeln!(out, "- {}", path.display());
    }
    if !group.errs.is_empty() {
        let _ = writeln!(out, "\n{}", texts.result_scan_issues);
        for err in &group.errs {
            let _ = writeln!(out, "- {err}");
        }
    }
    out
}
