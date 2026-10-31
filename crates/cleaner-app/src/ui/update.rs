//! Updates to Windows Cleaner itself: the header chip that points out a new
//! version, and the modal with its release notes, the install, and how often
//! to check. The checking, downloading, and swapping live in `app-update`.

use app_update::{CheckPeriod, NoteBlock, Settings, Update, Updater};
use eframe::egui::{self, RichText, Ui};

use crate::icons;
use crate::strings::UiText;
use crate::theme;
use crate::ui::components;

#[derive(Clone, Copy)]
pub(crate) enum UpdateAction {
    CheckNow,
    Download,
    Restart,
    Skip,
    Later,
    /// Closes the modal and leaves the reminder as it was.
    Close,
}

/// The header chip while a new version is out. Returns true when clicked.
pub(crate) fn chip(ui: &mut Ui, texts: &UiText, updater: &Updater, settings: &Settings) -> bool {
    let Some(release) = updater.notification(settings) else {
        return false;
    };
    let text = texts.update_chip(&release.version.to_string(), updater.staged().is_some());
    let button = egui::Button::new(
        RichText::new(icons::with_label(icons::UPDATE, &text))
            .family(theme::bold())
            .color(theme::ACCENT),
    )
    .fill(theme::ACCENT.gamma_multiply(0.16))
    .stroke(egui::Stroke::NONE)
    .corner_radius(theme::RADIUS_MD);
    ui.add(button)
        .on_hover_text(texts.update_chip_hint)
        .clicked()
}

pub(crate) fn show(
    ctx: &egui::Context,
    texts: &UiText,
    updater: &Updater,
    settings: &mut Settings,
    busy: bool,
    open: &mut bool,
) -> Option<UpdateAction> {
    if !*open {
        return None;
    }
    let mut action = None;
    let response = egui::Modal::new(egui::Id::unique("app-update")).show(ctx, |ui| {
        ui.set_max_width(540.0);
        ui.label(
            RichText::new(texts.update_title)
                .family(theme::bold())
                .size(theme::FONT_HEADING),
        );
        ui.separator();
        egui::ScrollArea::vertical()
            .max_height(380.0)
            .auto_shrink([false, true])
            .show(ui, |ui| status(ui, texts, updater, settings));
        if let Some(error) = updater.error() {
            ui.add_space(theme::SPACE_SM);
            ui.label(RichText::new(icons::with_label(icons::WARNING, error)).color(theme::DANGER));
        }
        ui.separator();
        if schedule(ui, texts, updater, settings) {
            action = Some(UpdateAction::CheckNow);
        }
        ui.separator();
        if let Some(chosen) = actions(ui, texts, updater, busy) {
            action = Some(chosen);
        }
    });
    if matches!(
        action,
        Some(UpdateAction::Skip | UpdateAction::Later | UpdateAction::Close)
    ) || response.should_close()
    {
        *open = false;
    }
    action
}

/// Where the app stands: a new version and its notes, a check in flight,
/// or nothing newer.
fn status(ui: &mut Ui, texts: &UiText, updater: &Updater, settings: &Settings) {
    let current = updater.current().map_or_else(
        || crate::APP_INFO.version.to_owned(),
        |version| version.to_string(),
    );
    if let Some(update) = updater.update() {
        available(ui, texts, updater, settings, update, &current);
    } else if updater.is_checking() {
        ui.horizontal(|ui| {
            ui.spinner();
            ui.label(texts.update_checking);
        });
    } else if updater.is_up_to_date() {
        ui.label(
            RichText::new(icons::with_label(
                icons::SUCCESS,
                &texts.update_latest(&current),
            ))
            .color(theme::ACCENT),
        );
    } else {
        ui.label(RichText::new(texts.update_not_checked(&current)).color(theme::MUTED));
    }
}

fn available(
    ui: &mut Ui,
    texts: &UiText,
    updater: &Updater,
    settings: &Settings,
    update: &Update,
    current: &str,
) {
    let latest = update.latest();
    let version = latest.version.to_string();
    ui.label(
        RichText::new(texts.update_available(&version))
            .family(theme::bold())
            .size(theme::FONT_BODY + 2.0),
    );
    ui.label(RichText::new(texts.update_current(current, &latest.date)).color(theme::MUTED));
    if updater.staged() == Some(latest.version) {
        ui.add_space(theme::SPACE_XS);
        ui.label(
            RichText::new(icons::with_label(icons::SUCCESS, texts.update_ready))
                .color(theme::ACCENT),
        );
    } else if settings.skipped.as_deref() == Some(version.as_str()) {
        ui.add_space(theme::SPACE_XS);
        ui.label(RichText::new(texts.update_skipped_note).color(theme::MUTED));
    }
    if !updater.can_install() {
        ui.add_space(theme::SPACE_XS);
        ui.label(RichText::new(texts.update_no_build).color(theme::MUTED));
        ui.hyperlink_to(
            icons::with_label(icons::EXTERNAL_LINK, texts.update_release_page),
            &latest.page_url,
        );
    }
    ui.add_space(theme::SPACE_MD);
    ui.label(RichText::new(texts.update_whats_new).family(theme::bold()));
    for release in update.releases() {
        ui.add_space(theme::SPACE_XS);
        ui.horizontal(|ui| {
            ui.label(RichText::new(release.version.to_string()).family(theme::bold()));
            if !release.date.is_empty() {
                ui.label(
                    RichText::new(&release.date)
                        .size(theme::FONT_SMALL)
                        .color(theme::MUTED),
                );
            }
        });
        notes(ui, texts, &release.notes);
    }
    ui.add_space(theme::SPACE_SM);
    ui.hyperlink_to(
        icons::with_label(icons::EXTERNAL_LINK, texts.update_all_releases),
        crate::APP_INFO.releases_page(),
    );
}

fn notes(ui: &mut Ui, texts: &UiText, markdown: &str) {
    let blocks = app_update::parse_notes(markdown);
    if blocks.is_empty() {
        ui.label(RichText::new(texts.update_no_notes).color(theme::MUTED));
    }
    for block in blocks {
        match block {
            NoteBlock::Heading(text) => {
                ui.label(RichText::new(text).family(theme::bold()));
            }
            NoteBlock::Bullet(text) => {
                ui.horizontal_top(|ui| {
                    ui.label(RichText::new("•").color(theme::MUTED));
                    ui.add(egui::Label::new(text).wrap());
                });
            }
            NoteBlock::Paragraph(text) => {
                ui.add(egui::Label::new(text).wrap());
            }
        }
    }
}

/// How often to check, and the button to check now. Returns true when
/// Check now was clicked.
fn schedule(ui: &mut Ui, texts: &UiText, updater: &Updater, settings: &mut Settings) -> bool {
    ui.label(RichText::new(texts.update_schedule_title).family(theme::bold()));
    let mut check = false;
    ui.horizontal(|ui| {
        ui.label(texts.update_period_label);
        egui::ComboBox::from_id_salt("app-update-period")
            .selected_text(settings.period.label())
            .show_ui(ui, |ui| {
                for option in CheckPeriod::ALL {
                    ui.selectable_value(&mut settings.period, option, option.label());
                }
            });
        check = ui
            .add_enabled(
                !updater.is_busy(),
                egui::Button::new(icons::with_label(icons::RESCAN, texts.update_check_now)),
            )
            .clicked();
    });
    let last = settings.last_check.map_or_else(
        || texts.update_never_checked.to_owned(),
        |then| texts.update_last_checked(&app_update::describe_age(then, app_update::now())),
    );
    ui.label(
        RichText::new(format!("{last}. {}", texts.update_privacy_note))
            .size(theme::FONT_SMALL)
            .color(theme::MUTED),
    );
    check
}

/// The bottom row: a progress bar while downloading, otherwise the actions
/// for the newest version, or just Close when there is none.
fn actions(ui: &mut Ui, texts: &UiText, updater: &Updater, busy: bool) -> Option<UpdateAction> {
    if let (Some((received, total)), Some(update)) = (updater.progress(), updater.update()) {
        let percent = total
            .filter(|total| *total > 0)
            .map_or(0, |total| received.saturating_mul(100) / total)
            .min(100);
        let fraction = f32::from(u8::try_from(percent).unwrap_or(100)) / 100.0;
        ui.add(
            egui::ProgressBar::new(fraction)
                .text(texts.update_downloading(&update.latest().version.to_string(), percent)),
        );
        return None;
    }
    let mut action = None;
    ui.horizontal(|ui| {
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if updater.update().is_none() {
                if ui.button(texts.dialog_close).clicked() {
                    action = Some(UpdateAction::Close);
                }
                return;
            }
            if updater.staged().is_some() {
                if ui
                    .add_enabled(!busy, components::accent_button(texts.update_restart))
                    .on_hover_text(texts.update_restart_hint)
                    .on_disabled_hover_text(texts.update_restart_busy)
                    .clicked()
                {
                    action = Some(UpdateAction::Restart);
                }
            } else if updater.can_install()
                && ui
                    .add_enabled(
                        !updater.is_busy(),
                        components::accent_button(&icons::with_label(
                            icons::DOWNLOAD,
                            texts.update_download,
                        )),
                    )
                    .on_hover_text(texts.update_download_hint)
                    .clicked()
            {
                action = Some(UpdateAction::Download);
            }
            if ui
                .button(texts.update_skip)
                .on_hover_text(texts.update_skip_hint)
                .clicked()
            {
                action = Some(UpdateAction::Skip);
            }
            if ui
                .button(texts.update_later)
                .on_hover_text(texts.update_later_hint)
                .clicked()
            {
                action = Some(UpdateAction::Later);
            }
        });
    });
    action
}
