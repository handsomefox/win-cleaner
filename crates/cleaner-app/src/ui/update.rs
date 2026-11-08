//! Updates to Windows Cleaner itself: the header chip that points out a new
//! version, and the modal around the shared update dialog. The checking,
//! downloading, and swapping live in `app-update`, and what the dialog says
//! lives in `app-update-ui`. This module maps the app's theme onto it.

use app_update::{Settings, Updater};
use app_update_ui::{Action, Icon};
use eframe::egui::{self, Color32, RichText, Ui, Visuals};

use crate::icons;
use crate::strings::UiText;
use crate::theme;
use crate::ui::components;

/// The app's theme for the shared update dialog.
struct Look;

impl app_update_ui::Look for Look {
    fn muted(&self, _visuals: &Visuals) -> Color32 {
        theme::MUTED
    }

    fn success(&self, _visuals: &Visuals) -> Color32 {
        theme::ACCENT
    }

    fn warning(&self, _visuals: &Visuals) -> Color32 {
        theme::DANGER
    }

    fn space_xs(&self) -> f32 {
        theme::SPACE_XS
    }

    fn space_sm(&self) -> f32 {
        theme::SPACE_SM
    }

    fn space_md(&self) -> f32 {
        theme::SPACE_MD
    }

    fn heading(&self, text: RichText) -> RichText {
        text.family(theme::bold()).size(theme::FONT_BODY + 2.0)
    }

    fn bold(&self, text: RichText) -> RichText {
        text.family(theme::bold())
    }

    fn fine_print(&self, text: RichText) -> RichText {
        text.size(theme::FONT_SMALL)
    }

    fn glyph(&self, icon: Icon) -> Option<&'static str> {
        match icon {
            Icon::CheckNow => Some(icons::RESCAN),
            Icon::Download => Some(icons::DOWNLOAD),
            Icon::Success => Some(icons::SUCCESS),
            Icon::Warning => Some(icons::WARNING),
            Icon::Link => Some(icons::EXTERNAL_LINK),
            Icon::Restart | Icon::Skip | Icon::Later => None,
        }
    }

    fn primary_button(&self, icon: Icon, text: &str) -> egui::Button<'static> {
        match self.glyph(icon) {
            Some(glyph) => components::accent_button(&icons::with_label(glyph, text)),
            None => components::accent_button(text),
        }
    }
}

/// The header chip while a new version is out. Returns true when clicked.
pub(crate) fn chip(ui: &mut Ui, updater: &Updater, settings: &Settings) -> bool {
    let Some(text) = app_update_ui::chip_text(updater, settings) else {
        return false;
    };
    let button = egui::Button::new(
        RichText::new(icons::with_label(icons::UPDATE, &text))
            .family(theme::bold())
            .color(theme::ACCENT),
    )
    .fill(theme::ACCENT.gamma_multiply(0.16))
    .stroke(egui::Stroke::NONE)
    .corner_radius(theme::RADIUS_MD);
    ui.add(button)
        .on_hover_text(app_update_ui::chip_hover_text(updater))
        .clicked()
}

/// The app updates modal. Returns what the user chose, for
/// [`app_update_ui::apply`]. `busy` locks the restart while a scan or cleanup
/// runs.
pub(crate) fn show(
    ctx: &egui::Context,
    texts: &UiText,
    updater: &Updater,
    settings: &mut Settings,
    busy: bool,
    open: &mut bool,
) -> Option<Action> {
    if !*open {
        return None;
    }
    let mut action = None;
    let mut close = false;
    let response = egui::Modal::new(egui::Id::unique("app-update")).show(ctx, |ui| {
        ui.set_max_width(540.0);
        ui.label(
            RichText::new(app_update_ui::TITLE)
                .family(theme::bold())
                .size(theme::FONT_HEADING),
        );
        ui.separator();
        action = app_update_ui::body(ui, &Look, updater, settings);
        let busy = busy.then_some(texts.update_restart_busy);
        action = app_update_ui::actions(ui, &Look, updater, busy).or(action);
        // The shared buttons cover a new version. Without one, the modal
        // still needs a way out besides the backdrop.
        if updater.update().is_none() {
            ui.separator();
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                close = ui.button(texts.dialog_close).clicked();
            });
        }
    });
    if close || response.should_close() {
        *open = false;
    }
    action
}
