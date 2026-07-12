//! Shared building blocks: surface cards, chips, tree rows, size cells, and
//! plain-text modals shared across the application screens.

use cleaner_core::{Group, GroupResult, human_bytes};
use eframe::egui::{
    self, Align2, Color32, FontId, Response, RichText, Sense, Ui, Vec2, WidgetInfo, WidgetType,
};

use crate::icons;
use crate::strings::UiText;
use crate::theme;

/// Rounded, bordered surface panel — the building block behind every card.
pub(crate) fn surface_frame() -> egui::Frame {
    egui::Frame::new()
        .fill(theme::SURFACE)
        .stroke(egui::Stroke::new(1.0, theme::BORDER))
        .corner_radius(theme::RADIUS_LG)
        .inner_margin(12)
}

/// Compact rounded pill used for header stats and progress figures.
pub(crate) fn chip_frame() -> egui::Frame {
    egui::Frame::new()
        .fill(theme::SURFACE_RAISED)
        .stroke(egui::Stroke::new(1.0, theme::BORDER))
        .corner_radius(theme::RADIUS_MD)
        .inner_margin(egui::Margin::symmetric(10, 6))
}

/// Tri-state of a checkbox aggregating a set of items.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum CheckState {
    Unchecked,
    Partial,
    Checked,
}

/// Derives the aggregate check state for `selected` of `total` items.
pub(crate) fn check_state(selected: usize, total: usize) -> CheckState {
    if total == 0 || selected == 0 {
        CheckState::Unchecked
    } else if selected == total {
        CheckState::Checked
    } else {
        CheckState::Partial
    }
}

/// The single tri-state checkbox used at every level (category, app, item).
/// A fixed square hit target keeps the checkbox column aligned regardless of
/// the glyph — this is the systematic alignment fix.
pub(crate) fn tri_checkbox(ui: &mut Ui, state: CheckState) -> Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(theme::CHECKBOX_HIT), Sense::click());
    if ui.is_rect_visible(rect) {
        if response.hovered() {
            ui.painter()
                .rect_filled(rect, theme::RADIUS_SM, theme::HOVER);
        }
        let (glyph, color) = match state {
            CheckState::Unchecked => (icons::CHECKBOX_UNCHECKED, theme::MUTED),
            CheckState::Partial => (icons::CHECKBOX_PARTIAL, theme::ACCENT),
            CheckState::Checked => (icons::CHECKBOX_CHECKED, theme::ACCENT),
        };
        ui.painter().text(
            rect.center(),
            Align2::CENTER_CENTER,
            glyph,
            FontId::proportional(theme::ICON_LG),
            color,
        );
    }
    let enabled = ui.is_enabled();
    let checked = state == CheckState::Checked;
    response.widget_info(|| WidgetInfo::selected(WidgetType::Checkbox, enabled, checked, ""));
    response
}

/// The one accent call-to-action button; fills [`theme::ACCENT`] with bold text
/// and a fixed control height.
pub(crate) fn accent_button(label: &str) -> egui::Button<'static> {
    egui::Button::new(
        RichText::new(label.to_owned())
            .family(theme::bold())
            .color(theme::TEXT),
    )
    .fill(theme::ACCENT)
    .min_size(egui::vec2(0.0, theme::CONTROL_HEIGHT))
}

/// A frameless glyph button with a hover tooltip.
pub(crate) fn icon_button(ui: &mut Ui, glyph: &str, tooltip: &str) -> Response {
    let response = ui.add(
        egui::Button::new(
            RichText::new(glyph)
                .size(theme::ICON_MD)
                .color(theme::MUTED),
        )
        .frame(false),
    );
    response.on_hover_text(tooltip)
}

/// A surface card with a bold title, muted subtitle, and body.
pub(crate) fn titled_card(ui: &mut Ui, title: &str, subtitle: &str, body: impl FnOnce(&mut Ui)) {
    surface_frame().show(ui, |ui| {
        ui.set_min_width(ui.available_width());
        ui.label(
            RichText::new(title)
                .family(theme::bold())
                .size(theme::FONT_HEADING),
        );
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

/// One item row: an optional zebra stripe behind a horizontal cluster spanning
/// the full width. Containment now comes from the enclosing card, so there is
/// no indent.
pub(crate) fn striped_row(ui: &mut Ui, striped: bool, add: impl FnOnce(&mut Ui)) {
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
            ui.horizontal(add);
        });
}

/// One sidebar row: a tinted, rounded button carrying an icon, a bold title,
/// an optional muted subtitle, and an optional right-aligned value.
pub(crate) fn sidebar_row(
    ui: &mut Ui,
    icon: &str,
    title: &str,
    subtitle: &str,
    value: Option<RichText>,
    selected: bool,
) -> Response {
    let (fill, tint) = if selected {
        (theme::SELECTION, theme::ACCENT)
    } else {
        (Color32::TRANSPARENT, theme::MUTED)
    };
    let response = egui::Frame::new()
        .fill(fill)
        .corner_radius(theme::RADIUS_MD)
        .inner_margin(egui::Margin::symmetric(10, 8))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.label(RichText::new(icon).size(theme::ICON_MD).color(tint));
                ui.vertical(|ui| {
                    ui.label(RichText::new(title).family(theme::bold()));
                    if !subtitle.is_empty() {
                        ui.label(
                            RichText::new(subtitle)
                                .size(theme::FONT_SMALL)
                                .color(theme::MUTED),
                        );
                    }
                });
                if let Some(value) = value {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(value);
                    });
                }
            });
        })
        .response;
    response.interact(Sense::click())
}

/// The glyph-prefixed outcome text for one executed group: failed count in
/// danger, or Skipped/OK muted.
pub(crate) fn group_status(texts: &UiText, group: &GroupResult) -> RichText {
    if group.paths_failed > 0 {
        RichText::new(icons::with_label(
            icons::ERROR,
            &texts.failed_count(group.paths_failed),
        ))
        .color(theme::DANGER)
    } else if group.paths_attempted == 0 {
        RichText::new(texts.result_status_skipped).color(theme::MUTED)
    } else {
        RichText::new(icons::with_label(icons::SUCCESS, texts.result_status_ok)).color(theme::MUTED)
    }
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
        ui.label(
            RichText::new(title)
                .family(theme::bold())
                .size(theme::FONT_HEADING),
        );
        ui.separator();
        egui::ScrollArea::vertical()
            .max_height(400.0)
            .show(ui, |ui| {
                // Detail bodies list paths and errors, so keep them copyable.
                ui.add(egui::Label::new(body).selectable(true));
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
