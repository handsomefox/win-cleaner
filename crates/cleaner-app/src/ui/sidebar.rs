//! The selection-screen sidebar: an "All" row, one row per category with its
//! app count and size, then the History and About actions.

use eframe::egui::{self, Ui};

use crate::app::SelectState;
use crate::icons;
use crate::strings::UiText;
use crate::theme;
use crate::ui::components;
use crate::viewmodel;

pub(crate) enum SidebarAction {
    History,
    About,
}

pub(crate) fn show(ui: &mut Ui, texts: &UiText, state: &mut SelectState) -> Option<SidebarAction> {
    let mut action = None;
    let summaries = viewmodel::category_summaries(&state.plan);
    let (total_apps, _items, total_bytes) = viewmodel::plan_overview(&state.plan);

    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            // "All" row selects the whole plan.
            if components::sidebar_row(
                ui,
                icons::CAT_ALL,
                texts.sidebar_all,
                &texts.apps_count(total_apps),
                Some(components::size_text(texts, total_bytes)),
                state.selected_category.is_none(),
            )
            .clicked()
            {
                state.selected_category = None;
            }

            for summary in &summaries {
                let selected = state.selected_category == Some(summary.category);
                if components::sidebar_row(
                    ui,
                    icons::category_glyph(summary.category),
                    summary.category.label(texts),
                    &texts.apps_count(summary.apps),
                    Some(components::size_text(texts, summary.bytes)),
                    selected,
                )
                .clicked()
                {
                    state.selected_category = Some(summary.category);
                }
            }

            ui.add_space(theme::SPACE_SM);
            ui.separator();
            ui.add_space(theme::SPACE_SM);

            if components::sidebar_row(ui, icons::HISTORY, texts.menu_history, "", None, false)
                .clicked()
            {
                action = Some(SidebarAction::History);
            }
            if components::sidebar_row(ui, icons::ABOUT, texts.menu_about, "", None, false)
                .clicked()
            {
                action = Some(SidebarAction::About);
            }
        });
    action
}
